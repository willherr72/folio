# Inject Folio's bounded system allocator without modifying upstream source.
function(folio_configure_jpeg)
  get_target_property(codec_sources jpeg-static SOURCES)
  list(FILTER codec_sources EXCLUDE REGEX "(^|/)jmemnobs\\.c$")
  list(APPEND codec_sources "${FOLIO_NATIVE_DIR}/bridge.c")
  set_property(TARGET jpeg-static PROPERTY SOURCES "${codec_sources}")
  target_include_directories(jpeg-static PRIVATE "${CMAKE_SOURCE_DIR}/src" "${CMAKE_BINARY_DIR}")
  set_target_properties(jpeg-static PROPERTIES
    OUTPUT_NAME folio_jpeg C_STANDARD 11 C_STANDARD_REQUIRED ON
    ARCHIVE_OUTPUT_DIRECTORY "${FOLIO_LIBRARY_DIR}"
    ARCHIVE_OUTPUT_DIRECTORY_RELEASE "${FOLIO_LIBRARY_DIR}")
endfunction()
cmake_language(DEFER CALL folio_configure_jpeg)
