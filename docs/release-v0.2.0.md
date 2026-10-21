Windows x64 用户优先下载名称以 -setup.exe 结尾的安装程序。安装后从开始菜单打开 MindRush，在模型设置中保存自己的接口地址、模型名和 API Key。

- -setup.exe：推荐安装方式，可按当前用户安装，自动处理 WebView2 运行环境（需要网络）。
- .msi：适合 Windows Installer 安装。
- MindRush-0.2.0-windows-x64-portable.zip：解压后双击 mindrush.exe；需要电脑已有 WebView2 Runtime。
- SHA256SUMS.txt：下载文件的 SHA256 校验值。

此次更新补齐源码构建与自动发布流程，恢复可构建的文档翻译界面，增加原生文件选择、中文配置引导、配置自动恢复、会话记录与复制译文，并修正缓存、EPUB 资源保留和 PDF 重试行为。

文本和文档会发送到用户所选模型接口；应用不附带模型或 API Key，服务费用由所选服务商收取。PDF 采用文本重排，不支持扫描件 OCR；复杂排版可能变化。

当前安装包未进行商业代码签名，Windows 可能提示未知发布者。请核对来源为本仓库并按需检查 SHA256。API Key 当前以明文保存在本机当前用户的配置目录，勿共享配置文件。
