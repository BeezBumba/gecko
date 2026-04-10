FROM rust:latest AS builder

RUN apt-get update && apt-get install -y clang lld --no-install-recommends && rm -rf /var/lib/apt/lists/* \
    && rustup target add wasm32-unknown-unknown \
    && curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /app
COPY . .

RUN wasm-pack build crates/web --target web --out-dir pkg --out-name gecko_web --release \
    && wasm-pack build crates/web --target web --out-dir pkg-dbg --out-name gecko_web --release -- --features debug

RUN mkdir -p /site/dbg /site/pkg /site/pkg-dbg \
    && cp crates/web/index.html /site/ \
    && cp -r crates/web/pkg/* /site/pkg/ \
    && cp crates/web/index.html /site/dbg/ \
    && sed -i 's|./pkg/gecko_web.js|../pkg-dbg/gecko_web.js|' /site/dbg/index.html \
    && cp -r crates/web/pkg-dbg/* /site/pkg-dbg/

FROM nginx:alpine
COPY --from=builder /site /usr/share/nginx/html
EXPOSE 80
