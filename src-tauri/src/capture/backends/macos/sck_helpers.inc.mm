static bool sck_copy_ax_point(AXUIElementRef element, CFStringRef attribute, CGPoint* out_point) {
    CFTypeRef value = nullptr;
    if (AXUIElementCopyAttributeValue(element, attribute, &value) != kAXErrorSuccess || !value) {
        return false;
    }

    bool success = CFGetTypeID(value) == AXValueGetTypeID()
        && AXValueGetType((AXValueRef)value) == (AXValueType)kAXValueCGPointType
        && AXValueGetValue((AXValueRef)value, (AXValueType)kAXValueCGPointType, out_point);
    CFRelease(value);
    return success;
}
static bool sck_copy_ax_size(AXUIElementRef element, CFStringRef attribute, CGSize* out_size) {
    CFTypeRef value = nullptr;
    if (AXUIElementCopyAttributeValue(element, attribute, &value) != kAXErrorSuccess || !value) {
        return false;
    }

    bool success = CFGetTypeID(value) == AXValueGetTypeID()
        && AXValueGetType((AXValueRef)value) == (AXValueType)kAXValueCGSizeType
        && AXValueGetValue((AXValueRef)value, (AXValueType)kAXValueCGSizeType, out_size);
    CFRelease(value);
    return success;
}

// Bring the selected window to the front before display-area capture begins.
// App activation works without Accessibility access. If access is available,
// AXRaise lets us raise the exact selected window rather than merely the app's
// most recently focused window.
static bool sck_raise_selected_window(CGWindowID window_id) {
    CFArrayRef window_info_ref = CGWindowListCopyWindowInfo(
        kCGWindowListOptionIncludingWindow,
        window_id);
    if (!window_info_ref) {
        NSLog(@"[SCK] Unable to look up selected window %u before capture", window_id);
        return false;
    }

    NSArray<NSDictionary*>* window_info = CFBridgingRelease(window_info_ref);
    NSDictionary* selected_info = window_info.firstObject;
    NSNumber* owner_pid_value = selected_info[(id)kCGWindowOwnerPID];
    NSDictionary* bounds_value = selected_info[(id)kCGWindowBounds];
    NSString* selected_title = selected_info[(id)kCGWindowName];
    CGRect selected_bounds = CGRectZero;

    if (!owner_pid_value || !bounds_value ||
        !CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)bounds_value, &selected_bounds)) {
        NSLog(@"[SCK] Selected window %u has incomplete WindowServer metadata", window_id);
        return false;
    }

    pid_t owner_pid = (pid_t)owner_pid_value.intValue;
    NSRunningApplication* application =
        [NSRunningApplication runningApplicationWithProcessIdentifier:owner_pid];
    if (!application) {
        NSLog(@"[SCK] Unable to resolve owning app for selected window %u", window_id);
        return false;
    }

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    BOOL activated = [application activateWithOptions:NSApplicationActivateIgnoringOtherApps];
#pragma clang diagnostic pop

    if (!AXIsProcessTrusted()) {
        NSLog(@"[SCK] Activated app for window %u; Accessibility access is unavailable, so exact AXRaise was skipped",
              window_id);
        return activated;
    }

    AXUIElementRef app_element = AXUIElementCreateApplication(owner_pid);
    if (!app_element) {
        return activated;
    }

    CFTypeRef windows_value = nullptr;
    AXError windows_error =
        AXUIElementCopyAttributeValue(app_element, kAXWindowsAttribute, &windows_value);
    if (windows_error != kAXErrorSuccess || !windows_value ||
        CFGetTypeID(windows_value) != CFArrayGetTypeID()) {
        if (windows_value) CFRelease(windows_value);
        CFRelease(app_element);
        NSLog(@"[SCK] Activated app for window %u, but could not enumerate its AX windows", window_id);
        return activated;
    }

    CFArrayRef ax_windows = (CFArrayRef)windows_value;
    AXUIElementRef best_window = nullptr;
    double best_score = INFINITY;

    for (CFIndex index = 0; index < CFArrayGetCount(ax_windows); index++) {
        AXUIElementRef candidate =
            (AXUIElementRef)CFArrayGetValueAtIndex(ax_windows, index);
        CGPoint position = CGPointZero;
        CGSize size = CGSizeZero;
        if (!sck_copy_ax_point(candidate, kAXPositionAttribute, &position) ||
            !sck_copy_ax_size(candidate, kAXSizeAttribute, &size)) {
            continue;
        }

        double score =
            fabs(position.x - selected_bounds.origin.x) +
            fabs(position.y - selected_bounds.origin.y) +
            fabs(size.width - selected_bounds.size.width) +
            fabs(size.height - selected_bounds.size.height);

        if (selected_title.length > 0) {
            CFTypeRef title_value = nullptr;
            if (AXUIElementCopyAttributeValue(candidate, kAXTitleAttribute, &title_value) == kAXErrorSuccess &&
                title_value && CFGetTypeID(title_value) == CFStringGetTypeID()) {
                if (![selected_title isEqualToString:(__bridge NSString*)title_value]) {
                    score += 10000.0;
                }
            }
            if (title_value) CFRelease(title_value);
        }

        if (score < best_score) {
            best_score = score;
            best_window = candidate;
        }
    }

    bool raised = false;
    // Bounds can differ slightly between WindowServer and Accessibility, but a
    // large mismatch means this is probably a different same-app window.
    if (best_window && best_score < 100.0) {
        AXUIElementSetAttributeValue(best_window, kAXMinimizedAttribute, kCFBooleanFalse);
        AXError raise_error = AXUIElementPerformAction(best_window, kAXRaiseAction);
        raised = raise_error == kAXErrorSuccess;
        if (raised) {
            AXUIElementSetAttributeValue(app_element, kAXFocusedWindowAttribute, best_window);
        } else {
            NSLog(@"[SCK] AXRaise failed for selected window %u (error=%d)",
                  window_id, (int)raise_error);
        }
    } else {
        NSLog(@"[SCK] No safe AX match for selected window %u (best score=%.1f)",
              window_id, best_score);
    }

    CFRelease(windows_value);
    CFRelease(app_element);
    return raised || activated;
}

static double sck_display_scale_factor(CGDirectDisplayID display_id) {
    double scale_factor = 1.0;
    CGDisplayModeRef mode = CGDisplayCopyDisplayMode(display_id);
    if (mode) {
        size_t pixel_width = CGDisplayModeGetPixelWidth(mode);
        size_t logical_width = CGDisplayModeGetWidth(mode);
        if (logical_width > 0) {
            scale_factor = (double)pixel_width / (double)logical_width;
        }
        CGDisplayModeRelease(mode);
    }
    return scale_factor;
}

// Persist the alpha channel from a one-time, window-only screenshot. The live
// recording remains a display-area stream.
static bool sck_save_window_silhouette(CGImageRef source, NSString* video_path) {
    if (!source || !video_path) {
        return false;
    }

    const size_t width = CGImageGetWidth(source);
    const size_t height = CGImageGetHeight(source);
    const size_t bytes_per_row = width * 4;
    uint8_t* pixels = static_cast<uint8_t*>(calloc(height, bytes_per_row));
    CGColorSpaceRef color_space = CGColorSpaceCreateDeviceRGB();
    CGContextRef context = CGBitmapContextCreate(
        pixels,
        width,
        height,
        8,
        bytes_per_row,
        color_space,
        kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big);
    CGColorSpaceRelease(color_space);

    if (!context || !pixels) {
        if (context) CGContextRelease(context);
        free(pixels);
        return false;
    }

    CGContextClearRect(context, CGRectMake(0, 0, width, height));
    CGContextDrawImage(context, CGRectMake(0, 0, width, height), source);
    for (size_t index = 0; index < width * height; index++) {
        uint8_t alpha = pixels[index * 4 + 3];
        pixels[index * 4] = alpha;
        pixels[index * 4 + 1] = alpha;
        pixels[index * 4 + 2] = alpha;
        pixels[index * 4 + 3] = 255;
    }

    CGImageRef mask_image = CGBitmapContextCreateImage(context);
    NSString* mask_path = [[video_path stringByDeletingPathExtension]
        stringByAppendingString:@".window-mask.png"];
    NSURL* mask_url = [NSURL fileURLWithPath:mask_path];
    CGImageDestinationRef destination = CGImageDestinationCreateWithURL(
        (__bridge CFURLRef)mask_url,
        CFSTR("public.png"),
        1,
        nullptr);
    bool saved = false;
    if (destination && mask_image) {
        CGImageDestinationAddImage(destination, mask_image, nullptr);
        saved = CGImageDestinationFinalize(destination);
    }

    if (destination) CFRelease(destination);
    if (mask_image) CGImageRelease(mask_image);
    CGContextRelease(context);
    free(pixels);

    if (saved) {
        NSLog(@"[SCK] Saved native window silhouette: %@ (%zux%zu)", mask_path, width, height);
    } else {
        NSLog(@"[SCK] Failed to save native window silhouette: %@", mask_path);
    }
    return saved;
}
