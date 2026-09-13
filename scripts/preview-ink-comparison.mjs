// Different rasterizers antialias curves differently. Check geometry, not byte equality.
export const inkTolerance = Object.freeze({ boundsPixels: 2, centroidPixels: 1,
  neighborPixels: 2, unmatchedFraction: 0.005, weightedAreaFraction: 0.2, minimumDarkness: 0.05 });

export function compareInk(width, height, actual, expected) {
  function measure(data) {
    const mask = new Uint8Array(width * height);
    let left = width, top = height, right = -1, bottom = -1, count = 0, area = 0, sumX = 0, sumY = 0;
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
      const pixel = y * width + x, i = pixel * 4;
      const darkness = (1 - (data[i] + data[i + 1] + data[i + 2]) / 765) * data[i + 3] / 255;
      if (darkness < inkTolerance.minimumDarkness) continue;
      mask[pixel] = 1; count++; area += darkness; sumX += x * darkness; sumY += y * darkness;
      left = Math.min(left, x); right = Math.max(right, x); top = Math.min(top, y); bottom = Math.max(bottom, y);
    }
    return { mask, count, area, bounds: [left, top, right, bottom], centroid: [sumX / area, sumY / area] };
  }
  const a = measure(actual), b = measure(expected);
  if (!a.count || !b.count) return { pass: false, reason: "Missing ink", actualCount: a.count, expectedCount: b.count };
  function unmatched(source, target) {
    let missing = 0;
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
      if (!source.mask[y * width + x] || target.mask[y * width + x]) continue;
      let found = false;
      const radius = inkTolerance.neighborPixels;
      for (let dy = -radius; dy <= radius && !found; dy++) for (let dx = -radius; dx <= radius; dx++) {
        if (x + dx >= 0 && x + dx < width && y + dy >= 0 && y + dy < height && target.mask[(y + dy) * width + x + dx]) { found = true; break; }
      }
      if (!found) missing++;
    }
    return missing / source.count;
  }
  const boundsDelta = Math.max(...a.bounds.map((value, index) => Math.abs(value - b.bounds[index])));
  const centroidDelta = Math.max(...a.centroid.map((value, index) => Math.abs(value - b.centroid[index])));
  const weightedAreaFraction = Math.abs(a.area - b.area) / b.area;
  const unmatchedFraction = Math.max(unmatched(a, b), unmatched(b, a));
  return { pass: boundsDelta <= inkTolerance.boundsPixels && centroidDelta <= inkTolerance.centroidPixels
    && weightedAreaFraction <= inkTolerance.weightedAreaFraction && unmatchedFraction <= inkTolerance.unmatchedFraction,
    boundsDelta, centroidDelta, weightedAreaFraction, unmatchedFraction,
    actualBounds: a.bounds, expectedBounds: b.bounds, actualCount: a.count, expectedCount: b.count };
}
