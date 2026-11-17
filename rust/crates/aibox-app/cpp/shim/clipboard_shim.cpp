#include "shim/clipboard_shim.h"

#include <QBuffer>
#include <QClipboard>
#include <QGuiApplication>
#include <QImage>

bool aibox_clipboard_set_text(const QString &text) {
    auto *cb = QGuiApplication::clipboard();
    if (!cb)
        return false;
    cb->setText(text);
    return true;
}

rust::Vec<uint8_t> aibox_clipboard_image_png() {
    rust::Vec<uint8_t> out;
    auto *cb = QGuiApplication::clipboard();
    if (!cb)
        return out;
    const QImage img = cb->image();
    if (img.isNull())
        return out;
    QByteArray png;
    QBuffer buffer(&png);
    buffer.open(QIODevice::WriteOnly);
    if (!img.save(&buffer, "PNG"))
        return out;
    out.reserve(static_cast<std::size_t>(png.size()));
    for (char c : png)
        out.push_back(static_cast<uint8_t>(c));
    return out;
}
