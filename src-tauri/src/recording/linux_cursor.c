/* A metadata-only consumer: do not map or copy video pixels. GStreamer's
 * pipewiresrc drops empty video buffers, including cursor-only updates. */
#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>
#include <spa/buffer/meta.h>
#include <stdlib.h>
#include <unistd.h>

typedef void (*cursor_callback)(uint32_t, uint32_t, uint32_t, uint32_t);
struct cursor_reader {
    struct pw_thread_loop *loop;
    struct pw_context *context;
    struct pw_core *core;
    struct pw_stream *stream;
    struct spa_hook listener;
    uint32_t width, height;
    int loop_started;
    cursor_callback callback;
};

static void cursor_process(void *userdata) {
    struct cursor_reader *reader = userdata;
    struct pw_buffer *buffer;
    while ((buffer = pw_stream_dequeue_buffer(reader->stream))) {
        struct spa_meta_cursor *cursor = spa_buffer_find_meta_data(
            buffer->buffer, SPA_META_Cursor, sizeof(struct spa_meta_cursor));
        if (cursor && reader->width && reader->height)
            reader->callback(reader->width, reader->height,
                cursor->id ? (uint32_t)cursor->position.x : UINT32_MAX,
                cursor->id ? (uint32_t)cursor->position.y : UINT32_MAX);
        pw_stream_queue_buffer(reader->stream, buffer);
    }
}

static void cursor_format(void *userdata, uint32_t id, const struct spa_pod *param) {
    struct cursor_reader *reader = userdata;
    if (id != SPA_PARAM_Format || !param) return;
    struct spa_video_info_raw info = {0};
    if (spa_format_video_raw_parse(param, &info) < 0) return;
    reader->width = info.size.width;
    reader->height = info.size.height;
    uint8_t storage[256];
    struct spa_pod_builder builder = SPA_POD_BUILDER_INIT(storage, sizeof(storage));
    const struct spa_pod *meta = spa_pod_builder_add_object(&builder,
        SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta,
        SPA_PARAM_META_type, SPA_POD_Id(SPA_META_Cursor),
        SPA_PARAM_META_size, SPA_POD_Int(sizeof(struct spa_meta_cursor) +
            sizeof(struct spa_meta_bitmap) + 256 * 256 * 4));
    pw_stream_update_params(reader->stream, &meta, 1);
}

static const struct pw_stream_events cursor_events = {
    PW_VERSION_STREAM_EVENTS,
    .param_changed = cursor_format,
    .process = cursor_process,
};

void tarantino_cursor_stop(struct cursor_reader *reader) {
    if (!reader) return;
    if (reader->loop_started) pw_thread_loop_stop(reader->loop);
    if (reader->stream) pw_stream_destroy(reader->stream);
    if (reader->core) pw_core_disconnect(reader->core);
    if (reader->context) pw_context_destroy(reader->context);
    if (reader->loop) pw_thread_loop_destroy(reader->loop);
    free(reader);
}

struct cursor_reader *tarantino_cursor_start(int portal_fd, uint32_t node,
                                             cursor_callback callback) {
    pw_init(NULL, NULL);
    struct cursor_reader *reader = calloc(1, sizeof(*reader));
    if (!reader) return NULL;
    reader->callback = callback;
    reader->loop = pw_thread_loop_new("tarantino-cursor", NULL);
    if (!reader->loop) goto failed;
    reader->context = pw_context_new(pw_thread_loop_get_loop(reader->loop), NULL, 0);
    if (!reader->context) goto failed;
    int fd = dup(portal_fd);
    if (fd < 0) goto failed;
    reader->core = pw_context_connect_fd(reader->context, fd, NULL, 0);
    if (!reader->core) goto failed; /* connect_fd consumes fd, including on error */
    reader->stream = pw_stream_new(reader->core, "Tarantino cursor metadata",
        pw_properties_new(PW_KEY_MEDIA_TYPE, "Video", PW_KEY_MEDIA_CATEGORY,
            "Capture", PW_KEY_MEDIA_ROLE, "Screen", NULL));
    if (!reader->stream) goto failed;
    pw_stream_add_listener(reader->stream, &reader->listener, &cursor_events, reader);
    uint8_t storage[512];
    struct spa_pod_builder builder = SPA_POD_BUILDER_INIT(storage, sizeof(storage));
    const struct spa_pod *format = spa_pod_builder_add_object(&builder,
        SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat,
        SPA_FORMAT_mediaType, SPA_POD_Id(SPA_MEDIA_TYPE_video),
        SPA_FORMAT_mediaSubtype, SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw),
        SPA_FORMAT_VIDEO_format, SPA_POD_CHOICE_ENUM_Id(4, SPA_VIDEO_FORMAT_BGRx,
            SPA_VIDEO_FORMAT_BGRA, SPA_VIDEO_FORMAT_RGBx, SPA_VIDEO_FORMAT_RGBA));
    if (pw_stream_connect(reader->stream, PW_DIRECTION_INPUT, node,
        PW_STREAM_FLAG_AUTOCONNECT | PW_STREAM_FLAG_DONT_RECONNECT, &format, 1) < 0)
        goto failed;
    if (pw_thread_loop_start(reader->loop) < 0) goto failed;
    reader->loop_started = 1;
    return reader;
failed:
    tarantino_cursor_stop(reader);
    return NULL;
}
