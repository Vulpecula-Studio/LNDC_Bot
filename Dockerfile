FROM rust:latest AS builder

# 创建工作目录
WORKDIR /app

# 安装构建依赖
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    wkhtmltopdf \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# 安装sccache加速编译
RUN cargo install sccache && \
    mkdir -p /root/.cache/sccache

# 设置代理加速依赖下载（取消注释以启用）
# ENV RUSTUP_DIST_SERVER=https://mirrors.tuna.tsinghua.edu.cn/rustup
# ENV CARGO_HTTP_MULTIPLEXING=false

# 设置Rust编译优化
ENV RUSTC_WRAPPER=sccache
ENV CARGO_INCREMENTAL=1
ENV RUST_BACKTRACE=1
ENV RUST_LOG=sccache=info
ENV SCCACHE_CACHE_SIZE=5G

# 先创建一个虚拟项目缓存依赖
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo "fn main() {println!(\"dummy\")}" > src/main.rs && \
    cargo fetch && \
    rm -rf src

# 复制整个项目
COPY . .

# 设置离线模式以使用缓存的依赖
ENV CARGO_NET_OFFLINE=true

# 设置并行编译参数
ENV CARGO_BUILD_JOBS=0 
# 0表示使用所有可用CPU核心

# 构建项目
# 如需静态链接OpenSSL（替代方案），取消注释以下行，注释掉下面的常规构建命令
# RUN OPENSSL_STATIC=1 OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu OPENSSL_INCLUDE_DIR=/usr/include/openssl cargo build --release
RUN cargo build --release

# 使用更小的基础镜像
FROM debian:bookworm-slim

# 设置工作目录
WORKDIR /app

# 安装依赖
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    wkhtmltopdf \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# 从构建阶段复制编译好的二进制文件
COPY --from=builder /app/target/release/rust_discord_bot .

# 复制字体文件和配置
COPY LXGWWenKaiGBScreen.ttf .
COPY .env.example .env

# 创建数据目录
RUN mkdir -p data/logs data/pic/temp data/sessions \
    && chmod +x rust_discord_bot

# 设置环境变量
ENV RUST_LOG=info

# 设置容器卷
VOLUME ["/app/data"]

# 运行应用
CMD ["./rust_discord_bot"] 