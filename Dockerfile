FROM rust:slim AS builder

# 创建工作目录
WORKDIR /app

# 安装构建依赖（精简版）
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    wkhtmltopdf \
    && rm -rf /var/lib/apt/lists/*

# 设置并行编译参数
ENV CARGO_BUILD_JOBS=4
# 设置为实际的CPU核心数量，例如4

# 先创建一个虚拟项目缓存依赖
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo "fn main() {println!(\"dummy\")}" > src/main.rs && \
    cargo fetch && \
    rm -rf src

# 复制整个项目
COPY . .

# 构建项目
RUN cargo build --release && \
    strip target/release/rust_discord_bot

# 使用更小的基础镜像
FROM debian:bookworm-slim

# 设置工作目录
WORKDIR /app

# 安装运行时依赖（精简版）
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    wkhtmltopdf \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# 从构建阶段复制编译好的二进制文件
COPY --from=builder /app/target/release/rust_discord_bot .

# 确保assets/fonts目录存在，并复制字体文件到正确位置
RUN mkdir -p assets/fonts
COPY assets/fonts/LXGWWenKaiGBScreen.ttf assets/fonts/
# 确保字体文件权限正确
RUN chmod 644 assets/fonts/LXGWWenKaiGBScreen.ttf

# 安装字体工具并注册字体（让wkhtmltoimage可以识别）
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    fontconfig && \
    fc-cache -fv && \
    rm -rf /var/lib/apt/lists/*

# 复制配置
COPY .env.example .env

# 确保.env中的路径设置正确
RUN sed -i 's|FONT_PATHS=.*|FONT_PATHS=./assets/fonts/LXGWWenKaiGBScreen.ttf|g' .env

# 创建数据目录
RUN mkdir -p data/logs data/pic/temp data/sessions \
    && chmod +x rust_discord_bot

# 设置环境变量
ENV RUST_LOG=info

# 设置容器卷
VOLUME ["/app/data"]

# 运行应用
CMD ["./rust_discord_bot"] 