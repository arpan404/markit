#ifndef MARKIT_H
#define MARKIT_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct { uint8_t *data; size_t len; uint32_t status; } MarkitBuffer;
/* format 0: native JSON, 1: safe HTML, 2: {source,language} code request JSON.
   Format 3 uses optional full grammars; status 2 if not compiled in.
   Input is borrowed for this call only.
   Nonempty input must point to len readable UTF-8 bytes; NULL,0 is allowed.
   status 0: success, 1: invalid UTF-8, 2: invalid argument, 3: panic.
   Output is length-delimited, not NUL-terminated. Free exactly once. */
MarkitBuffer markit_render(const uint8_t *source, size_t len, uint32_t format);
void markit_buffer_free(MarkitBuffer buffer);
typedef struct MarkitStream MarkitStream;
MarkitStream *markit_stream_new(void);
/* One caller at a time per stream. Output ownership matches markit_render. */
MarkitBuffer markit_stream_append(MarkitStream *stream, const uint8_t *source, size_t len, uint8_t finalized);
void markit_stream_free(MarkitStream *stream);
#ifdef __cplusplus
}
#endif
#endif
