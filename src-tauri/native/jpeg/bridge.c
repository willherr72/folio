/* Folio's bounded coefficient interface and libjpeg system allocation backend.
   No filename IO; all recovery and longjmp boundaries stay in this C module. */
#define JPEG_INTERNALS
#include "jinclude.h"
#include "jpeglib.h"
#include "jmemsys.h"
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>

typedef struct {
  uint32_t width, height, components, hmax, vmax, transformed, scans;
  size_t memory_limit, output_limit;
  uint64_t work_limit;
} FolioJpegRequest;
typedef struct {
  unsigned char *data;
  size_t length, peak_memory, retained_memory;
} FolioJpegResult;
typedef struct Context Context;
typedef union Allocation Allocation;
union Allocation {
  struct { Allocation *next, *previous; size_t bytes; Context *owner; } h;
#if defined(_MSC_VER)
  /* MSVC's C headers omit max_align_t. Match the platform malloc alignment. */
#if defined(_WIN64)
  __declspec(align(16)) unsigned char alignment;
#else
  __declspec(align(8)) unsigned char alignment;
#endif
#else
  max_align_t alignment;
#endif
};
struct Context {
  struct jpeg_decompress_struct source;
  struct jpeg_compress_struct target;
  struct jpeg_error_mgr source_error, target_error;
  struct jpeg_destination_mgr destination;
  struct jpeg_progress_mgr progress;
  jmp_buf jump;
  FolioJpegRequest request;
  Allocation *allocations;
  size_t current, peak, output_capacity, output_length;
  unsigned char *output;
  unsigned char overflow_byte;
  int status, at_output_limit;
};

static void fail(Context *ctx, int status) {
  ctx->status = status;
  longjmp(ctx->jump, 1);
}
static void *allocate(Context *ctx, size_t bytes) {
  Allocation *a;
  if (bytes > SIZE_MAX - sizeof(Allocation) ||
      bytes + sizeof(Allocation) > ctx->request.memory_limit - ctx->current) return NULL;
  a = (Allocation *)malloc(bytes + sizeof(Allocation));
  if (!a) return NULL;
  a->h.bytes = bytes + sizeof(Allocation); a->h.owner = ctx;
  a->h.previous = NULL; a->h.next = ctx->allocations;
  if (a->h.next) a->h.next->h.previous = a;
  ctx->allocations = a; ctx->current += a->h.bytes;
  if (ctx->peak < ctx->current) ctx->peak = ctx->current;
  return a + 1;
}
static void unlink_allocation(Context *ctx, Allocation *a) {
  if (a->h.previous) a->h.previous->h.next = a->h.next;
  else ctx->allocations = a->h.next;
  if (a->h.next) a->h.next->h.previous = a->h.previous;
  ctx->current -= a->h.bytes;
  a->h.owner = NULL; a->h.next = a->h.previous = NULL;
}
static void release(Context *ctx, void *data) {
  Allocation *a;
  if (!data) return;
  a = ((Allocation *)data) - 1;
  unlink_allocation(ctx, a); free(a);
}
GLOBAL(void *) jpeg_get_small(j_common_ptr info, size_t bytes) {
  return allocate((Context *)info->client_data, bytes);
}
GLOBAL(void *) jpeg_get_large(j_common_ptr info, size_t bytes) {
  return allocate((Context *)info->client_data, bytes);
}
GLOBAL(void) jpeg_free_small(j_common_ptr info, void *data, size_t bytes) {
  (void)bytes; release((Context *)info->client_data, data);
}
GLOBAL(void) jpeg_free_large(j_common_ptr info, void *data, size_t bytes) {
  (void)bytes; release((Context *)info->client_data, data);
}
GLOBAL(size_t) jpeg_mem_available(j_common_ptr info, size_t minimum, size_t maximum, size_t allocated) {
  Context *ctx = (Context *)info->client_data;
  size_t remaining = ctx->request.memory_limit - ctx->current;
  (void)minimum; (void)allocated;
  return remaining < maximum ? remaining : maximum;
}
GLOBAL(void) jpeg_open_backing_store(j_common_ptr info, backing_store_ptr store, long size) {
  (void)store; (void)size; fail((Context *)info->client_data, 2);
}
GLOBAL(long) jpeg_mem_init(j_common_ptr info) { (void)info; return 0; }
GLOBAL(void) jpeg_mem_term(j_common_ptr info) { (void)info; }
static void codec_error(j_common_ptr info) {
  fail((Context *)info->client_data, info->err->msg_code == JERR_OUT_OF_MEMORY ? 2 : 4);
}
static void codec_message(j_common_ptr info, int level) {
  if (level < 0) fail((Context *)info->client_data, 4);
}
static void codec_output(j_common_ptr info) { (void)info; }
static void progress(j_common_ptr info) {
  Context *ctx = (Context *)info->client_data;
  if (info->is_decompressor && ctx->source.input_scan_number > (int)ctx->request.scans) fail(ctx, 1);
}
static void destination_init(j_compress_ptr info) {
  Context *ctx = (Context *)info->client_data;
  ctx->output_capacity = ctx->request.output_limit < 4096 ? ctx->request.output_limit : 4096;
  ctx->output = allocate(ctx, ctx->output_capacity);
  if (!ctx->output) fail(ctx, 2);
  ctx->destination.next_output_byte = ctx->output;
  ctx->destination.free_in_buffer = ctx->output_capacity;
}
static boolean destination_grow(j_compress_ptr info) {
  Context *ctx = (Context *)info->client_data;
  size_t next = ctx->output_capacity > ctx->request.output_limit / 2 ? ctx->request.output_limit : ctx->output_capacity * 2;
  unsigned char *replacement;
  if (next <= ctx->output_capacity) {
    /* Libjpeg asks for another buffer as soon as the final byte fills this
       one. A context-owned sentinel distinguishes exact fit from overflow. */
    if (ctx->at_output_limit) fail(ctx, 3);
    ctx->at_output_limit = 1;
    ctx->destination.next_output_byte = &ctx->overflow_byte;
    ctx->destination.free_in_buffer = 1;
    return TRUE;
  }
  replacement = allocate(ctx, next);
  if (!replacement) fail(ctx, 2);
  memcpy(replacement, ctx->output, ctx->output_capacity);
  release(ctx, ctx->output); ctx->output = replacement;
  ctx->destination.next_output_byte = replacement + ctx->output_capacity;
  ctx->destination.free_in_buffer = next - ctx->output_capacity;
  ctx->output_capacity = next;
  return TRUE;
}
static void destination_finish(j_compress_ptr info) {
  Context *ctx = (Context *)info->client_data;
  if (ctx->at_output_limit) {
    if (ctx->destination.free_in_buffer != 1) fail(ctx, 3);
    ctx->output_length = ctx->output_capacity;
  } else ctx->output_length = ctx->output_capacity - ctx->destination.free_in_buffer;
}
static void cleanup(Context *ctx) {
  if (ctx->target.mem) jpeg_destroy_compress(&ctx->target);
  if (ctx->source.mem) jpeg_destroy_decompress(&ctx->source);
  /* Also cover allocations made before libjpeg linked a partially initialized
     structure into one of its pools. */
  while (ctx->allocations) release(ctx, ctx->allocations + 1);
}

int folio_jpeg_rewrite(const unsigned char *input, size_t length,
    const FolioJpegRequest *request, const unsigned char *mask, size_t mask_length,
    FolioJpegResult *result) {
  Context *ctx;
  if (!result) return 1;
  memset(result, 0, sizeof(*result));
  if (!input || !request || !mask || !length || length > 64u * 1024u * 1024u ||
      !request->width || !request->height || request->width > 32768 || request->height > 32768 ||
      (uint64_t)request->width * request->height > 16000000 ||
      (request->components != 1 && request->components != 3) || request->transformed > 1 ||
      !request->hmax || request->hmax > 2 || !request->vmax || request->vmax > 2 ||
      !request->scans || request->scans > 64 || !request->output_limit || request->output_limit > 64u*1024u*1024u ||
      request->memory_limit < sizeof(Context) || request->memory_limit > 192u*1024u*1024u ||
      !request->work_limit || request->work_limit > 512000000) return 1;
  ctx = calloc(1, sizeof(*ctx));
  if (!ctx) return 2;
  ctx->request = *request; ctx->current = ctx->peak = sizeof(*ctx);
  if (setjmp(ctx->jump)) {
    int status = ctx->status;
    cleanup(ctx); result->peak_memory = ctx->peak;
    result->retained_memory = ctx->current - sizeof(*ctx);
    free(ctx); return status;
  }
  ctx->source.err = jpeg_std_error(&ctx->source_error);
  ctx->source_error.error_exit = codec_error; ctx->source_error.emit_message = codec_message; ctx->source_error.output_message = codec_output;
  ctx->source.client_data = ctx;
  jpeg_create_decompress(&ctx->source);
  jpeg_mem_src(&ctx->source, input, (unsigned long)length);
  if (jpeg_read_header(&ctx->source, TRUE) != JPEG_HEADER_OK || ctx->source.data_precision != 8 ||
      ctx->source.image_width != request->width || ctx->source.image_height != request->height ||
      ctx->source.num_components != (int)request->components || ctx->source.max_h_samp_factor != (int)request->hmax ||
      ctx->source.max_v_samp_factor != (int)request->vmax || ctx->source.arith_code) fail(ctx, 1);
  {
    uint64_t columns = (request->width + 8 * request->hmax - 1) / (8 * request->hmax);
    uint64_t rows = (request->height + 8 * request->vmax - 1) / (8 * request->vmax);
    uint64_t blocks = 0;
    size_t n;
    int ci, used_tables = 0;
    jvirt_barray_ptr *arrays;
    if (columns * rows != mask_length) fail(ctx, 1);
    for (n = 0; n < mask_length; n++) if (mask[n] > 1) fail(ctx, 1);
    for (ci = 0; ci < ctx->source.num_components; ci++) {
      jpeg_component_info *comp = &ctx->source.comp_info[ci];
      if (comp->h_samp_factor < 1 || comp->h_samp_factor > 2 || comp->v_samp_factor < 1 || comp->v_samp_factor > 2) fail(ctx, 1);
      blocks += columns * rows * comp->h_samp_factor * comp->v_samp_factor;
    }
    if (blocks * 64 * ((uint64_t)request->scans + 3) > request->work_limit) fail(ctx, 1);
    ctx->progress.progress_monitor = progress; ctx->source.progress = &ctx->progress;
    arrays = jpeg_read_coefficients(&ctx->source);
    if (!arrays || ctx->source.input_scan_number != (int)request->scans) fail(ctx, 1);
    ctx->target.err = jpeg_std_error(&ctx->target_error);
    ctx->target_error.error_exit = codec_error; ctx->target_error.emit_message = codec_message; ctx->target_error.output_message = codec_output;
    ctx->target.client_data = ctx; jpeg_create_compress(&ctx->target);
    ctx->destination.init_destination = destination_init;
    ctx->destination.empty_output_buffer = destination_grow;
    ctx->destination.term_destination = destination_finish;
    ctx->target.dest = &ctx->destination;
    jpeg_copy_critical_parameters(&ctx->source, &ctx->target);
    ctx->target.write_JFIF_header = ctx->source.saw_JFIF_marker;
    ctx->target.write_Adobe_marker = ctx->source.saw_Adobe_marker;
    ctx->target.JFIF_major_version = 1; ctx->target.JFIF_minor_version = 1;
    ctx->target.density_unit = 0; ctx->target.X_density = ctx->target.Y_density = 1;
    for (ci = 0; ci < ctx->source.num_components; ci++) {
      jpeg_component_info *comp = &ctx->source.comp_info[ci];
      JQUANT_TBL *table = ctx->source.quant_tbl_ptrs[comp->quant_tbl_no];
      int k, dc;
      JDIMENSION y, x;
      if (!table) fail(ctx, 1);
      for (k = 0; k < 64; k++) if (!table->quantval[k] || table->quantval[k] > 255) fail(ctx, 1);
      used_tables |= 1 << comp->quant_tbl_no;
      dc = request->transformed && ci != 0 ? 0 : -(int)((1024 + table->quantval[0] - 1) / table->quantval[0]);
      for (y = 0; y < comp->height_in_blocks; y++) {
        JBLOCKARRAY row = (*ctx->source.mem->access_virt_barray)((j_common_ptr)&ctx->source, arrays[ci], y, 1, TRUE);
        for (x = 0; x < comp->width_in_blocks; x++) {
          size_t index = (size_t)(y / comp->v_samp_factor) * (size_t)columns + x / comp->h_samp_factor;
          if (index >= mask_length) fail(ctx, 1);
          if (mask[index]) { memset(row[0][x], 0, sizeof(JBLOCK)); row[0][x][0] = (JCOEF)dc; }
        }
      }
    }
    for (ci = 0; ci < NUM_QUANT_TBLS; ci++) if (!(used_tables & (1 << ci))) ctx->target.quant_tbl_ptrs[ci] = NULL;
    jpeg_write_coefficients(&ctx->target, arrays);
    jpeg_finish_compress(&ctx->target);
    if (!jpeg_finish_decompress(&ctx->source) || ctx->source.src->bytes_in_buffer != 0 || !ctx->output_length) fail(ctx, 1);
  }
  result->data = ctx->output; result->length = ctx->output_length;
  unlink_allocation(ctx, ((Allocation *)ctx->output) - 1);
  cleanup(ctx); result->peak_memory = ctx->peak;
  result->retained_memory = ctx->current - sizeof(*ctx);
  free(ctx); return 0;
}
void folio_jpeg_release(unsigned char *data) {
  if (data) free(((Allocation *)data) - 1);
}
