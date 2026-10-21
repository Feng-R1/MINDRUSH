# MindRush

**在自己的电脑上，用自己选择的 AI 服务翻译文档。**

EPUB · PDF · Markdown · 文本 ｜ Rust + Tauri 2 + React

[下载 Windows 版本](https://github.com/Feng-R1/MINDRUSH/releases/latest) · [报告问题](https://github.com/Feng-R1/MINDRUSH/issues)

MindRush 是一款开源桌面翻译应用。它支持文本及整本 EPUB、PDF、Markdown 文档翻译，并尽量保留文档结构和排版。应用不提供或代理 AI 服务：你配置自己的模型接口，文本会直接发送到你选择的服务商。

## 安装与使用

### Windows 安装包

1. 打开 [Releases 下载页](https://github.com/Feng-R1/MINDRUSH/releases/latest)，下载 .exe 或 .msi 安装包。
2. 安装并启动 MindRush。
3. 打开「模型设置」，选择 OpenAI 兼容接口、Anthropic 或本地接口，填写模型名称及对应信息并保存。
4. 粘贴文本并翻译。整书翻译可在应用中选择 EPUB、PDF 或 Markdown 文档。

如果尚无已发布安装包，可在仓库的 Actions 页面运行 **Windows release** 工作流并下载构建产物，或从源码构建。

### 从源码构建

需要 Node.js 20.19 或更新版本，或 Node.js 22.12 或更新版本，以及 Rust stable 和 Windows 桌面构建工具（Microsoft C++ Build Tools、WebView2 Runtime 和 NSIS）。在 Windows PowerShell 中运行：

    git clone https://github.com/Feng-R1/MINDRUSH.git
    cd MINDRUSH
    npm ci --prefix frontend
    npm run desktop:build -- --bundles nsis,msi

安装包位于 target/release/bundle/。

开发模式运行：

    cd MINDRUSH
    npm ci --prefix frontend
    npm run desktop:dev

## 支持的模型接口

- **OpenAI 兼容接口**：配置 API Key 与模型名，可选填写兼容接口的基础地址；支持 OpenAI Chat Completions 格式的服务。
- **Anthropic**：配置 Anthropic API Key 和模型名；基础地址可留空。
- **本地 / 自定义接口**：当前作为 OpenAI 兼容接口使用。选择此项，填写模型名及兼容的基础地址，API Key 可留空。

MindRush 不内置模型，也不承担第三方模型费用。价格、数据保留方式和可用地区由所选服务商决定。API Key 保存在本机应用配置中，不要提交到仓库。

> 当前配置以 JSON 文件保存在本机，其中包含 API Key。请保护当前用户账户，避免把配置目录同步到公开位置。

## 主要功能

- 翻译英文、中文、日语、韩语、法语、德语、西班牙语、俄语、葡萄牙语和意大利语等语言。
- 翻译 EPUB、PDF 与 Markdown 文档，尽量保留章节、段落、代码、图片及原有文档结构。
- 分批处理长文档，并对限流和超时请求重试。
- 配置术语表和系统提示词，统一专有名词译法。
- 缓存重复翻译；缓存区分语言方向、模型和翻译设置。

## 发布版本

维护者发布版本时，先同步更新根目录 package.json、frontend/package.json、Cargo.toml 和 src-tauri/tauri.conf.json 中的版本，再推送匹配的 tag：

    git tag vX.Y.Z
    git push origin vX.Y.Z

GitHub Actions 会在 Windows 上构建 NSIS .exe 和 MSI .msi 安装包，并附加到 GitHub Release；CI 会检查版本与标签的一致性。

## 技术栈

- 后端：Rust、Tauri 2
- 前端：React、TypeScript、Vite
- 文档：rbook、pdf_oxide、pulldown-cmark

## 开发检查

    cd frontend
    npm ci
    npm run build
    cd ..
    cargo test --workspace

Windows 安装包需要 Windows 构建环境；其他平台需要对应平台的 Tauri 构建环境。

## 许可证

本项目采用 MIT License，详见 LICENSE。
