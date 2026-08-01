#import <Foundation/Foundation.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <CoreGraphics/CoreGraphics.h>
#import <ImageIO/ImageIO.h>
#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>

#include <stdint.h>
#include <stdbool.h>
#include <math.h>
#include <unistd.h>

typedef struct {
    uint64_t display_id;
    const char* name;
    uint32_t width;
    uint32_t height;
    double scale_factor;
    bool is_primary;
} SCKDisplay;

typedef struct {
    uint64_t window_id;
    const char* title;
    int32_t x;
    int32_t y;
    uint32_t width;
    uint32_t height;
    const char* owner_name;
} SCKWindow;

typedef struct {
    bool screen_recording;
    bool microphone;
    bool camera;
} SCKPermissionStatus;

typedef struct {
    uint64_t source_id;
    bool is_display;
    uint32_t fps;
    bool include_cursor;
    bool include_audio;
    uint32_t crop_x;
    uint32_t crop_y;
    uint32_t crop_width;
    uint32_t crop_height;
    const char* output_path; // optional; unused in current flow
} SCKCaptureConfig;

typedef struct {
    const uint8_t* data;
    size_t data_len;
    uint32_t width;
    uint32_t height;
    const char* pixel_format;
    uint64_t timestamp_us;
    uint32_t stride;
} SCKFrameData;

typedef struct {
    const uint8_t* data;
    size_t data_len;
    uint32_t sample_rate;
    uint32_t channels;
    uint64_t timestamp_us;
} SCKAudioData;

// Frame callback function pointer type
typedef void (*SCKFrameCallbackFn)(void* context, SCKFrameData frame);

// Audio callback function pointer type
typedef void (*SCKAudioCallbackFn)(void* context, SCKAudioData audio);

// Context structure to hold the callbacks
typedef struct {
    void* rust_context;
    SCKFrameCallbackFn frame_callback;
    SCKAudioCallbackFn audio_callback;
} SCKCallbackContext;

// Forward declaration of capture session class
#include "sck_capture_session.inc.mm"

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

extern "C" {

// Check if ScreenCaptureKit is available
bool sck_is_available() {
    if (@available(macOS 12.3, *)) {
        return true;
    }
    return false;
}

// Get shareable displays
bool sck_get_shareable_displays(SCKDisplay** out_displays, size_t* out_count) {
    if (@available(macOS 12.3, *)) {
        __block bool success = false;
        __block NSArray<SCDisplay*>* displays = nil;

        dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);

        [SCShareableContent getShareableContentExcludingDesktopWindows:YES
                                                    onScreenWindowsOnly:YES
                                                      completionHandler:^(SCShareableContent* content, NSError* error) {
            if (error) {
                NSLog(@"Failed to get shareable content: %@", error);
            } else {
                displays = content.displays;
                success = true;
            }
            dispatch_semaphore_signal(semaphore);
        }];

        if (dispatch_semaphore_wait(semaphore, dispatch_time(DISPATCH_TIME_NOW, 3 * NSEC_PER_SEC)) != 0) {
            NSLog(@"[SCK] Timed out getting shareable displays");
            *out_count = 0;
            return false;
        }

        if (!success || !displays) {
            *out_count = 0;
            return false;
        }

        *out_count = displays.count;
        *out_displays = (SCKDisplay*)malloc(sizeof(SCKDisplay) * displays.count);

        for (size_t i = 0; i < displays.count; i++) {
            SCDisplay* display = displays[i];
            CGRect frame = display.frame;

            NSString* displayName = [NSString stringWithFormat:@"Display %u", display.displayID];

            double scale_factor = 1.0;
            CGDisplayModeRef mode = CGDisplayCopyDisplayMode(display.displayID);
            if (mode) {
                size_t pixelWidth = CGDisplayModeGetPixelWidth(mode);
                size_t logicalWidth = CGDisplayModeGetWidth(mode);
                if (logicalWidth > 0) {
                    scale_factor = (double)pixelWidth / (double)logicalWidth;
                }
                CGDisplayModeRelease(mode);
            }

            (*out_displays)[i] = (SCKDisplay){
                .display_id = display.displayID,
                .name = strdup([displayName UTF8String]),
                .width = (uint32_t)frame.size.width,
                .height = (uint32_t)frame.size.height,
                .scale_factor = scale_factor,
                .is_primary = (i == 0) // First display is usually primary
            };
        }

        return true;
    }

    *out_count = 0;
    return false;
}

// Get shareable windows
bool sck_get_shareable_windows(SCKWindow** out_windows, size_t* out_count) {
    if (@available(macOS 12.3, *)) {
        __block bool success = false;
        __block NSArray<SCWindow*>* windows = nil;

        dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);

        [SCShareableContent getShareableContentExcludingDesktopWindows:YES
                                                    onScreenWindowsOnly:NO
                                                      completionHandler:^(SCShareableContent* content, NSError* error) {
            if (error) {
                NSLog(@"Failed to get shareable content: %@", error);
            } else {
                windows = content.windows;
                success = true;
            }
            dispatch_semaphore_signal(semaphore);
        }];

        if (dispatch_semaphore_wait(semaphore, dispatch_time(DISPATCH_TIME_NOW, 3 * NSEC_PER_SEC)) != 0) {
            NSLog(@"[SCK] Timed out getting shareable windows");
            *out_count = 0;
            return false;
        }

        if (!success || !windows) {
            *out_count = 0;
            return false;
        }

        NSMutableArray<SCWindow*>* filtered = [NSMutableArray array];
        for (SCWindow* window in windows) {
            if (window.windowLayer != 0) continue;
            if (!window.owningApplication) continue;
            if (!window.title || window.title.length == 0) continue;
            CGRect frame = window.frame;
            if (frame.size.width < 1 || frame.size.height < 1) continue;
            [filtered addObject:window];
        }

        NSLog(@"[SCK] Shareable windows: raw=%lu filtered=%lu", (unsigned long)windows.count, (unsigned long)filtered.count);

        if (filtered.count == 0) {
            NSMutableArray<NSDictionary*>* cgFiltered = [NSMutableArray array];
            CFArrayRef windowInfo = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID);
            if (windowInfo) {
                NSArray* cgWindows = CFBridgingRelease(windowInfo);
                for (NSDictionary* info in cgWindows) {
                    NSNumber* layerNumber = info[(id)kCGWindowLayer];
                    if (!layerNumber || layerNumber.integerValue != 0) continue;
                    NSNumber* windowNumber = info[(id)kCGWindowNumber];
                    NSString* ownerName = info[(id)kCGWindowOwnerName];
                    NSString* windowName = info[(id)kCGWindowName];
                    NSDictionary* bounds = info[(id)kCGWindowBounds];
                    if (!windowNumber || !ownerName || !bounds) continue;
                    CGRect frame = CGRectZero;
                    if (!CGRectMakeWithDictionaryRepresentation((CFDictionaryRef)bounds, &frame)) continue;
                    if (frame.size.width < 1 || frame.size.height < 1) continue;
                    NSDictionary* normalized = @{
                        @"id": windowNumber, @"owner": ownerName,
                        @"title": (windowName && windowName.length > 0) ? windowName : ownerName,
                        @"x": @(frame.origin.x), @"y": @(frame.origin.y),
                        @"width": @(frame.size.width), @"height": @(frame.size.height)
                    };
                    [cgFiltered addObject:normalized];
                }
            }
            NSLog(@"[SCK] CGWindowList fallback windows=%lu", (unsigned long)cgFiltered.count);
            *out_count = cgFiltered.count;
            *out_windows = (SCKWindow*)malloc(sizeof(SCKWindow) * cgFiltered.count);
            for (size_t i = 0; i < cgFiltered.count; i++) {
                NSDictionary* window = cgFiltered[i];
                (*out_windows)[i] = (SCKWindow){
                    .window_id = [window[@"id"] unsignedLongLongValue],
                    .title = strdup([window[@"title"] UTF8String] ?: "Untitled"),
                    .x = [window[@"x"] intValue],
                    .y = [window[@"y"] intValue],
                    .width = [window[@"width"] unsignedIntValue],
                    .height = [window[@"height"] unsignedIntValue],
                    .owner_name = strdup([window[@"owner"] UTF8String] ?: "Unknown")
                };
            }

            return true;
        }

        *out_count = filtered.count;
        *out_windows = (SCKWindow*)malloc(sizeof(SCKWindow) * filtered.count);

        for (size_t i = 0; i < filtered.count; i++) {
            SCWindow* window = filtered[i];
            CGRect frame = window.frame;

            (*out_windows)[i] = (SCKWindow){
                .window_id = window.windowID,
                .title = strdup([window.title UTF8String] ?: "Untitled"),
                .x = (int32_t)frame.origin.x,
                .y = (int32_t)frame.origin.y,
                .width = (uint32_t)frame.size.width,
                .height = (uint32_t)frame.size.height,
                .owner_name = strdup([window.owningApplication.applicationName UTF8String] ?: "Unknown")
            };
        }

        return true;
    }

    *out_count = 0;
    return false;
}

// Check permissions
SCKPermissionStatus sck_check_permissions() {
    SCKPermissionStatus status = {false, false, false};

    // Check screen recording permission
    if (@available(macOS 10.15, *)) {
        // Use CGPreflightScreenCaptureAccess to check without prompting
        status.screen_recording = CGPreflightScreenCaptureAccess();
    }

    // TODO: Check microphone and camera permissions
    // For now, assume not granted

    return status;
}

// Request permissions
SCKPermissionStatus sck_request_permissions() {
    SCKPermissionStatus status = {false, false, false};

    // Request screen recording permission
    if (@available(macOS 10.15, *)) {
        // CGRequestScreenCaptureAccess triggers the system prompt if needed
        status.screen_recording = CGRequestScreenCaptureAccess();
    }

    // TODO: Request microphone and camera permissions if needed

    return status;
}

// Free display array
void sck_free_displays(SCKDisplay* displays, size_t count) {
    if (!displays) return;

    for (size_t i = 0; i < count; i++) {
        if (displays[i].name) {
            free((void*)displays[i].name);
        }
    }

    free(displays);
}

// Free window array
void sck_free_windows(SCKWindow* windows, size_t count) {
    if (!windows) return;

    for (size_t i = 0; i < count; i++) {
        if (windows[i].title) {
            free((void*)windows[i].title);
        }
        if (windows[i].owner_name) {
            free((void*)windows[i].owner_name);
        }
    }

    free(windows);
}

// Start capture
void* sck_start_capture(SCKCaptureConfig config, void* rust_context, SCKFrameCallbackFn frame_callback, SCKAudioCallbackFn audio_callback) {
    if (@available(macOS 12.3, *)) {
        __block SCKCaptureSession* session = nil;
        __block bool success = false;
        __block bool cancelled = false;
        __block const char* startupPhase = "requesting shareable content";

        dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);

        NSLog(@"[SCK] start_capture begin source=%llu type=%@", config.source_id, config.is_display ? @"display" : @"window");

        if (!config.is_display) {
            bool foregrounded = sck_raise_selected_window((CGWindowID)config.source_id);
            if (foregrounded) {
                // Give WindowServer a short startup-only interval to commit the
                // activation/raise before taking the fresh geometry snapshot.
                usleep(150000);
            }
        }

        // Get shareable content (onScreenWindowsOnly:NO to find windows on all screens)
        [SCShareableContent getShareableContentExcludingDesktopWindows:YES
                                                    onScreenWindowsOnly:NO
                                                      completionHandler:^(SCShareableContent* content, NSError* error) {
            startupPhase = "processing shareable content";
            if (cancelled) {
                NSLog(@"[SCK] Ignoring late shareable-content callback after start timeout");
                return;
            }

            if (error) {
                NSLog(@"Failed to get shareable content: %@", error);
                dispatch_semaphore_signal(semaphore);
                return;
            }

            // Find the target display or window
            SCContentFilter* filter = nil;
            SCDisplay* targetDisplay = nil;
            SCWindow* targetWindow = nil;
            CGRect windowSourceRect = CGRectNull;
            double outputScaleFactor = 1.0;

            if (config.is_display) {
                for (SCDisplay* display in content.displays) {
                    if (display.displayID == config.source_id) {
                        targetDisplay = display;
                        break;
                    }
                }

                if (!targetDisplay) {
                    NSLog(@"Display not found: %llu", config.source_id);
                    dispatch_semaphore_signal(semaphore);
                    return;
                }

                filter = [[SCContentFilter alloc] initWithDisplay:targetDisplay
                                                  excludingWindows:@[]];
            } else {
                for (SCWindow* window in content.windows) {
                    if (window.windowID == config.source_id) {
                        targetWindow = window;
                        break;
                    }
                }

                if (!targetWindow) {
                    NSLog(@"Window not found: %llu", config.source_id);
                    dispatch_semaphore_signal(semaphore);
                    return;
                }

                NSLog(@"[SCK] Target window found: id=%u app=%@ title=%@ frame=%.0fx%.0f",
                      targetWindow.windowID,
                      targetWindow.owningApplication.applicationName,
                      targetWindow.title,
                      targetWindow.frame.size.width,
                      targetWindow.frame.size.height);

                CGRect windowFrame = CGRectStandardize(targetWindow.frame);
                double windowArea = windowFrame.size.width * windowFrame.size.height;
                double largestIntersectionArea = 0.0;

                for (SCDisplay* display in content.displays) {
                    CGRect intersection = CGRectIntersection(windowFrame, display.frame);
                    if (CGRectIsNull(intersection) || CGRectIsEmpty(intersection)) {
                        continue;
                    }

                    double intersectionArea =
                        intersection.size.width * intersection.size.height;
                    if (intersectionArea > largestIntersectionArea) {
                        largestIntersectionArea = intersectionArea;
                        targetDisplay = display;
                    }
                }

                if (!targetDisplay) {
                    NSLog(@"[SCK] No display contains selected window %u", targetWindow.windowID);
                    dispatch_semaphore_signal(semaphore);
                    return;
                }

                // A single display-area stream cannot reproduce a window split
                // across displays without clipping it. Fail instead of silently
                // recording the wrong rectangle.
                if (largestIntersectionArea + 1.0 < windowArea) {
                    NSLog(@"[SCK] Selected window %u spans displays (visible %.0f/%.0f points); move it onto one display",
                          targetWindow.windowID, largestIntersectionArea, windowArea);
                    dispatch_semaphore_signal(semaphore);
                    return;
                }

                CGRect displayFrame = targetDisplay.frame;
                windowSourceRect = CGRectMake(
                    windowFrame.origin.x - displayFrame.origin.x,
                    windowFrame.origin.y - displayFrame.origin.y,
                    windowFrame.size.width,
                    windowFrame.size.height);

                const double coordinateTolerance = 0.01;
                if (windowSourceRect.origin.x < -coordinateTolerance ||
                    windowSourceRect.origin.y < -coordinateTolerance ||
                    CGRectGetMaxX(windowSourceRect) > displayFrame.size.width + coordinateTolerance ||
                    CGRectGetMaxY(windowSourceRect) > displayFrame.size.height + coordinateTolerance) {
                    NSLog(@"[SCK] Refusing invalid local crop for window %u: (%.2f,%.2f %.2fx%.2f) outside display %.2fx%.2f",
                          targetWindow.windowID,
                          windowSourceRect.origin.x, windowSourceRect.origin.y,
                          windowSourceRect.size.width, windowSourceRect.size.height,
                          displayFrame.size.width, displayFrame.size.height);
                    dispatch_semaphore_signal(semaphore);
                    return;
                }

                // Normalize harmless floating-point noise at display edges.
                windowSourceRect.origin.x = MAX(0.0, windowSourceRect.origin.x);
                windowSourceRect.origin.y = MAX(0.0, windowSourceRect.origin.y);
                windowSourceRect.size.width = MIN(
                    windowSourceRect.size.width,
                    displayFrame.size.width - windowSourceRect.origin.x);
                windowSourceRect.size.height = MIN(
                    windowSourceRect.size.height,
                    displayFrame.size.height - windowSourceRect.origin.y);
                outputScaleFactor = sck_display_scale_factor(targetDisplay.displayID);

                // Capture the selected window's real alpha silhouette without
                // blocking the live stream startup callback. This supports
                // standard, square, and app-specific window shapes.
                if (@available(macOS 14.0, *)) {
                    if (config.output_path) {
                        NSString* silhouetteOutputPath =
                            [NSString stringWithUTF8String:config.output_path];
                        SCContentFilter* silhouetteFilter =
                            [[SCContentFilter alloc] initWithDesktopIndependentWindow:targetWindow];
                        SCStreamConfiguration* silhouetteConfig =
                            [[SCStreamConfiguration alloc] init];
                        silhouetteConfig.width = MAX(
                            (size_t)1,
                            (size_t)llround(windowFrame.size.width * outputScaleFactor));
                        silhouetteConfig.height = MAX(
                            (size_t)1,
                            (size_t)llround(windowFrame.size.height * outputScaleFactor));
                        silhouetteConfig.showsCursor = NO;
                        silhouetteConfig.ignoreShadowsSingleWindow = YES;
                        [SCScreenshotManager
                            captureImageWithFilter:silhouetteFilter
                            configuration:silhouetteConfig
                            completionHandler:^(CGImageRef image, NSError* screenshotError) {
                                if (screenshotError || !image) {
                                    NSLog(@"[SCK] Window silhouette capture failed: %@",
                                          screenshotError);
                                    return;
                                }
                                sck_save_window_silhouette(image, silhouetteOutputPath);
                            }];
                    }
                }

                filter = [[SCContentFilter alloc] initWithDisplay:targetDisplay
                                                  excludingWindows:@[]];
                NSLog(@"[SCK] Window-as-area display=%u global=(%.1f,%.1f %.1fx%.1f) local=(%.1f,%.1f %.1fx%.1f) scale=%.2f",
                      targetDisplay.displayID,
                      windowFrame.origin.x, windowFrame.origin.y,
                      windowFrame.size.width, windowFrame.size.height,
                      windowSourceRect.origin.x, windowSourceRect.origin.y,
                      windowSourceRect.size.width, windowSourceRect.size.height,
                      outputScaleFactor);
            }

            // Configure stream dimensions
            SCStreamConfiguration* streamConfig = [[SCStreamConfiguration alloc] init];
            if (!config.is_display && targetWindow && targetDisplay && !CGRectIsNull(windowSourceRect)) {
                streamConfig.sourceRect = windowSourceRect;
                streamConfig.width = MAX((size_t)1, (size_t)llround(windowSourceRect.size.width * outputScaleFactor));
                streamConfig.height = MAX((size_t)1, (size_t)llround(windowSourceRect.size.height * outputScaleFactor));
                NSLog(@"[SCK] Using window-area pixel dimensions: %zux%zu",
                      streamConfig.width, streamConfig.height);
            } else if (config.crop_width > 0 && config.crop_height > 0) {
                streamConfig.width = config.crop_width;
                streamConfig.height = config.crop_height;
            } else if (targetDisplay) {
                // Display: use physical pixel dimensions for Retina
                CGRect displayFrame = targetDisplay.frame;
                double scale_factor = sck_display_scale_factor(targetDisplay.displayID);
                streamConfig.width = (size_t)(displayFrame.size.width * scale_factor);
                streamConfig.height = (size_t)(displayFrame.size.height * scale_factor);
                NSLog(@"[SCK] Using display pixel dimensions: %zux%zu (scale: %.1f)",
                      streamConfig.width, streamConfig.height, scale_factor);
            } else {
                streamConfig.width = 1920;
                streamConfig.height = 1080;
            }
            streamConfig.minimumFrameInterval = CMTimeMake(1, config.fps);
            streamConfig.pixelFormat = kCVPixelFormatType_32BGRA;
            // Always hide system cursor - we render our own overlay cursor in the editor
            // This allows cursor settings (style, hide, rotation, click effects) to work
            streamConfig.showsCursor = NO;

            // Disable Presenter Overlay alerts where supported. This setting
            // doesn't control macOS's system recording indicator.
#if __MAC_OS_X_VERSION_MAX_ALLOWED >= 140000
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability-new"
            if (@available(macOS 14.0, *)) {
                streamConfig.presenterOverlayPrivacyAlertSetting =
                    SCPresenterOverlayAlertSettingNever;
            }
#pragma clang diagnostic pop
#endif

            streamConfig.scalesToFit = YES;

            // Increase queue depth to reduce frame drops under memory pressure
            // Keep within ScreenCaptureKit's documented practical limit.
            streamConfig.queueDepth = 8;

            // Enable audio capture if requested (macOS 13+)
#if __MAC_OS_X_VERSION_MAX_ALLOWED >= 130000
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability"
            if (config.include_audio) {
                streamConfig.capturesAudio = YES;
            }
#pragma clang diagnostic pop
#endif

            // Create callback context
            SCKCallbackContext* ctx = (SCKCallbackContext*)malloc(sizeof(SCKCallbackContext));
            ctx->rust_context = rust_context;
            ctx->frame_callback = frame_callback;
            ctx->audio_callback = audio_callback;

            // Create capture session
            session = [[SCKCaptureSession alloc] init];
            session.filter = filter;
            session.config = streamConfig;
            session.callbackContext = ctx;

            // Initialize frame statistics
            session.totalFramesReceived = 0;
            session.framesDroppedNoBuffer = 0;
            session.lastStatsLogTime = 0;
            session.startupFailed = NO;
            session.startupSemaphore = semaphore;

            // Create stream
            startupPhase = "creating stream";
            NSError* streamError = nil;
            session.stream = [[SCStream alloc] initWithFilter:filter
                                                 configuration:streamConfig
                                                      delegate:session];

            if (streamError) {
                NSLog(@"Failed to create stream: %@", streamError);
                dispatch_semaphore_signal(semaphore);
                return;
            }

            // Create dedicated serial queue with USER_INTERACTIVE QoS for frame handling
            // This reduces frame drops by ensuring consistent, high-priority processing
            dispatch_queue_attr_t frameQueueAttr = dispatch_queue_attr_make_with_qos_class(
                DISPATCH_QUEUE_SERIAL, QOS_CLASS_USER_INTERACTIVE, 0);
            dispatch_queue_t frameQueue = dispatch_queue_create("com.tarantino.frameQueue", frameQueueAttr);

            // Add stream output for video
            startupPhase = "adding video output";
            NSError* outputError = nil;
            [session.stream addStreamOutput:session
                                       type:SCStreamOutputTypeScreen
                         sampleHandlerQueue:frameQueue
                                      error:&outputError];

            if (outputError) {
                NSLog(@"Failed to add video stream output: %@", outputError);
                dispatch_semaphore_signal(semaphore);
                return;
            }

            // Add stream output for audio if enabled (macOS 13+)
#if __MAC_OS_X_VERSION_MAX_ALLOWED >= 130000
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability"
            if (config.include_audio) {
                // Create dedicated queue for audio as well
                dispatch_queue_attr_t audioQueueAttr = dispatch_queue_attr_make_with_qos_class(
                    DISPATCH_QUEUE_SERIAL, QOS_CLASS_USER_INTERACTIVE, 0);
                dispatch_queue_t audioQueue = dispatch_queue_create("com.tarantino.audioQueue", audioQueueAttr);

                NSError* audioOutputError = nil;
                [session.stream addStreamOutput:session
                                           type:SCStreamOutputTypeAudio
                             sampleHandlerQueue:audioQueue
                                          error:&audioOutputError];

                if (audioOutputError) {
                    NSLog(@"Failed to add audio stream output: %@", audioOutputError);
                    // Continue anyway - video capture will still work
                }
            }
#pragma clang diagnostic pop
#endif

            // Start capture
            startupPhase = "waiting for startCapture completion";
            [session.stream startCaptureWithCompletionHandler:^(NSError* error) {
                if (cancelled) {
                    if (session && session.stream) {
                        [session.stream stopCaptureWithCompletionHandler:nil];
                    }
                    if (session && session.callbackContext) {
                        free(session.callbackContext);
                        session.callbackContext = nullptr;
                    }
                    NSLog(@"[SCK] Ignoring late start-capture callback after start timeout");
                    return;
                }
                if (error) {
                    NSLog(@"Failed to start capture: %@", error);
                    success = false;
                    session.startupFailed = YES;
                    session.startupSemaphore = nil;
                    dispatch_semaphore_signal(semaphore);
                } else {
                    session.isCapturing = YES;
                    success = true;
                    session.startupSemaphore = nil;
                    NSLog(@"[SCK] startCapture completion succeeded");
                    dispatch_semaphore_signal(semaphore);
                }
            }];
        }];

        if (dispatch_semaphore_wait(semaphore, dispatch_time(DISPATCH_TIME_NOW, 20 * NSEC_PER_SEC)) != 0) {
            cancelled = true;
            NSLog(@"[SCK] Timed out starting capture during phase: %s", startupPhase);
            return nullptr;
        }

        if (success && session && session.isCapturing && !session.startupFailed) {
            return (__bridge_retained void*)session;
        }

        if (session) {
            if (session.stream && session.isCapturing) {
                [session.stream stopCaptureWithCompletionHandler:nil];
            }
            session.isCapturing = NO;
            session.startupSemaphore = nil;
            if (session.callbackContext) {
                free(session.callbackContext);
                session.callbackContext = nullptr;
            }
            session.stream = nil;
        }
    }

    return nullptr;
}

// Stop capture
bool sck_stop_capture(void* instance, void** out_rust_context) {
    if (!instance) {
        return false;
    }

    if (@available(macOS 12.3, *)) {
        SCKCaptureSession* session = (__bridge_transfer SCKCaptureSession*)instance;

        if (session.stream && session.isCapturing) {
            __block bool stopped = false;
            dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);

            [session.stream stopCaptureWithCompletionHandler:^(NSError* error) {
                if (error) {
                    NSLog(@"Failed to stop capture: %@", error);
                } else {
                    stopped = true;
                }
                dispatch_semaphore_signal(semaphore);
            }];

            dispatch_semaphore_wait(semaphore, DISPATCH_TIME_FOREVER);

            session.isCapturing = NO;

            // Log final statistics
            if (session.totalFramesReceived > 0) {
                double dropRate = (double)session.framesDroppedNoBuffer / (double)session.totalFramesReceived;
                NSLog(@"[SCK STATS] Capture session ended - Total frames: %llu, Dropped: %llu (%.2f%%)",
                      session.totalFramesReceived, session.framesDroppedNoBuffer, dropRate * 100.0);
            }

            session.stream = nil;

            // Extract rust_context and return it to Rust for proper cleanup
            if (session.callbackContext) {
                if (out_rust_context) {
                    *out_rust_context = session.callbackContext->rust_context;
                }
                // Free the C callback context struct (but not the rust_context inside it)
                free(session.callbackContext);
                session.callbackContext = nullptr;
            } else if (out_rust_context) {
                *out_rust_context = nullptr;
            }

            return stopped;
        }

        if (session.callbackContext) {
            if (out_rust_context) {
                *out_rust_context = session.callbackContext->rust_context;
            }
            free(session.callbackContext);
            session.callbackContext = nullptr;
        } else if (out_rust_context) {
            *out_rust_context = nullptr;
        }

        session.stream = nil;
        return true;
    }

    return false;
}

} // extern "C"
