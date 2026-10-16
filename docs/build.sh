#!/usr/bin/env bash
#
# Builds the documentation site into target/site:
#
#   target/site/index.html   the landing page
#   target/site/assets/      the shared styles, scripts and icon
#   target/site/book/        the manual (mdBook)
#   target/site/api/         the Rust API reference (rustdoc)
#
# Requires mdBook (https://rust-lang.github.io/mdBook/). Set MDBOOK to the path
# of the mdbook binary if it is not on the PATH.
#
# To preview the site:  python3 -m http.server -d target/site

set -euo pipefail

cd "$(dirname "$0")/.."
site=target/site
mdbook=${MDBOOK:-mdbook}

rm -rf "$site"
mkdir -p "$site"

# Landing page and shared assets
cp docs/site/index.html "$site/"
cp -r docs/assets "$site/assets"

# Manual
"$mdbook" build docs
cp -r target/book "$site/book"

# Rust API reference, with the site's look and navigation. It is built in its
# own target folder, so the extra rustdoc flags do not invalidate the normal
# build cache.
theme=target/rustdoc-theme
mkdir -p "$theme"
{
    echo "<style>"
    cat docs/assets/texl.css docs/assets/rustdoc.css
    echo "</style>"
    echo '<link rel="icon" href="https://onlabsorg.github.io/texl/assets/favicon.svg" type="image/svg+xml">'
} > "$theme/in-header.html"

# Cargo does not notice changes to the files passed to rustdoc: always rebuild.
cargo clean --doc --target-dir target/rustdoc

RUSTDOCFLAGS="--html-in-header $PWD/$theme/in-header.html --html-before-content $PWD/docs/rustdoc/before-content.html" \
    cargo doc --no-deps -p swan-core -p texl-core --target-dir target/rustdoc

cp -r target/rustdoc/doc "$site/api"
cp docs/rustdoc/index.html "$site/api/index.html"

echo "Site built in $site"
