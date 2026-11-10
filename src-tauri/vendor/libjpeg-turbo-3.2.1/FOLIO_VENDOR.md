# Folio native JPEG codec source

Unmodified official libjpeg-turbo 3.2.1 release source. Archive SHA-256: c27ad72a68da151177929aa969ae773a2da5ac2af9ce946079314fdac8cece7f, verified against upstream release asset metadata. See FOLIO_PROVENANCE.json for the official URL and every upstream source hash.

Folio builds the static libjpeg API with TurboJPEG, SIMD and arithmetic coding disabled. A separate CMake wrapper replaces the system allocation backend with Folio's tracked native bridge; upstream source is unmodified. The new bridge is outside this vendor directory.

Retain LICENSE.md and README.ijg with distribution. This software is based in part on the work of the Independent JPEG Group.
