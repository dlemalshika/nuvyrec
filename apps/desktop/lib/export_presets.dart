/// Local H.264 export profiles. These are Frame defaults, not upload-service
/// requirements. Dimensions are limits, except for padded social canvases.
enum ExportPreset {
  original('Original size', 'Keep the source size and frame rate.', 0, 0, 20),
  youtube(
    'YouTube / landscape',
    'Up to 1920 × 1080, 30 fps; preserve aspect ratio.',
    1920,
    1080,
    21,
  ),
  vertical(
    'Vertical / stories',
    '1080 × 1920 canvas, 30 fps; black padding, no cropping.',
    1080,
    1920,
    23,
    padded: true,
  ),
  square(
    'Square / social',
    '1080 × 1080 canvas, 30 fps; black padding, no cropping.',
    1080,
    1080,
    23,
    padded: true,
  ),
  compact(
    'Small file',
    'Up to 1280 × 720, 30 fps; more compression.',
    1280,
    720,
    28,
  );

  const ExportPreset(
    this.label,
    this.description,
    this.width,
    this.height,
    this.crf, {
    this.padded = false,
  });
  final String label, description;
  final int width, height, crf;
  final bool padded;

  /// Never enlarges source content. Scale normalizes non-square source pixels;
  /// fixed social canvases pad after fitting rather than discarding content.
  /// Crop runs before this filter, so the chosen region is fitted as a whole.
  List<String> filters(double? sourceFps) {
    if (this == original) {
      return ['scale=trunc(iw/2)*2:trunc(ih/2)*2'];
    }
    return [
      "scale=w='min($width,iw*sar)':h='min($height,ih)':force_original_aspect_ratio=decrease:force_divisible_by=2:reset_sar=1",
      if (padded) 'pad=$width:$height:(ow-iw)/2:(oh-ih)/2:color=black',
      // Decimate high-rate recordings; never invent frames for slower sources.
      if (sourceFps != null && sourceFps.isFinite && sourceFps > 30) 'fps=30',
    ];
  }
}

enum ExportAudio {
  all('All audio tracks'),
  first('First audio track (player friendly)'),
  muted('No audio');

  const ExportAudio(this.label);
  final String label;
}
