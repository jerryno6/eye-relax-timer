#!/bin/bash

# Thoát ngay nếu có lệnh nào bị lỗi
set -e

# Kiểm tra tham số version được truyền vào
if [ -z "$1" ]; then
  echo "❌ Lỗi: Vui lòng cung cấp version mới."
  echo "💡 Sử dụng: ./scripts/bump-version.sh <version_mới>"
  echo "💡 Ví dụ: ./scripts/bump-version.sh 0.1.2"
  exit 1
fi

NEW_VERSION=$1

# Bỏ chữ 'v' ở đầu nếu người dùng vô tình nhập vào (vd: v0.1.2 -> 0.1.2)
NEW_VERSION=${NEW_VERSION#v}

echo "🚀 Đang cập nhật version của dự án lên: $NEW_VERSION"

# 1. Cập nhật version trong package.json và package-lock.json
echo "📦 Đang cập nhật package.json..."
# Sử dụng --allow-same-version phòng trường hợp chạy lại nhiều lần
npm version "$NEW_VERSION" --no-git-tag-version --allow-same-version

# 2. Chạy script đồng bộ hóa version sang tauri.conf.json và Cargo.toml
# Lưu ý: npm run sync-version sẽ gọi script node có sẵn ở scripts/sync-version.cjs
echo "🔄 Đang đồng bộ version sang Tauri và Cargo..."
npm run sync-version

echo ""
echo "✅ Cập nhật version thành công trên tất cả các file!"
echo ""
echo "Các file đã bị thay đổi:"
git status -s | grep -E "package.*\.json|tauri\.conf\.json|Cargo\.toml" || true
echo ""
echo "🚀 QUY TRÌNH RELEASE TỰ ĐỘNG:"
echo "Dự án của bạn đã có sẵn CI/CD tự động tạo tag (tag.yml). Bạn chỉ cần commit và push lên nhánh main:"
echo ""
echo "  git add ."
echo "  git commit -m \"chore: release v$NEW_VERSION\""
echo "  git push origin main"
echo ""
echo "Hệ thống sẽ tự động tạo tag v$NEW_VERSION và build macOS Universal artifact trên GitHub Releases!"
