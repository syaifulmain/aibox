#pragma once

#include "rust/cxx.h"
#include <QString>

bool aibox_clipboard_set_text(const QString &text);
rust::Vec<uint8_t> aibox_clipboard_image_png();
