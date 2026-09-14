<div align="center">

<img src="src-tauri/icons/icon.svg" width="88" alt="OpenHistory 图标">

# OpenHistory

<p>把工作上下文留在自己的电脑里。</p>

<p>
  <a href="https://github.com/wellorbetter/open-history/actions/workflows/check.yml"><img alt="检查状态" src="https://img.shields.io/github/actions/workflow/status/wellorbetter/open-history/check.yml?branch=main&amp;style=flat-square&amp;labelColor=242938&amp;color=80dfb7&amp;label=checks"></a>
  <a href="https://tauri.app"><img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-7fc9ed?style=flat-square&amp;labelColor=242938&amp;logo=tauri&amp;logoColor=white"></a>
  <img alt="macOS 与 Windows" src="https://img.shields.io/badge/platform-macOS_%2F_Windows-baa7f5?style=flat-square&amp;labelColor=242938">
  <a href="README.md"><img alt="英文与简体中文文档" src="https://img.shields.io/badge/docs-English_%2F_%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-e5c181?style=flat-square&amp;labelColor=242938"></a>
</p>

<p><a href="README.md">English</a> · <strong>简体中文</strong></p>
<p><a href="#为什么做-openhistory">为什么</a> · <a href="#隐私优先">隐私</a> · <a href="#开发">开发</a></p>

</div>

![OpenHistory 的 macOS 菜单栏紧凑界面](docs/openhistory-preview.png)

OpenHistory 是面向 macOS 和 Windows 的开源、本地优先活动时间线。它把经用户同意的语义
辅助功能事件整理成确定、可恢复的工作上下文；不截屏、不录音，也不记录原始按键。

> **Private Alpha：** 当前已有界面样例、领域核心、隐私边界和跨平台未签名构建。原生采集与
> 生产级存储仍在完善中；暂时不要把这个版本当作日常活动记录器。

## 为什么做 OpenHistory

电脑使用历史属于创造它的人，不应受产品地区开放范围或云账户限制。OpenHistory 把主记录
保存在本机；只有经过单独授权，其他工具才能读取范围受限的上下文。

| 一眼看懂       | 具体含义                                       |
| :------------- | :--------------------------------------------- |
| **简洁**       | macOS 菜单栏弹窗与 Windows 通知区域浮窗。      |
| **实用**       | 当前任务、每日时间线、搜索、修正、总结与导出。 |
| **本地优先**   | 确定性处理离线可用，模型增强完全可选。         |
| **面向 Agent** | 版本化本地 API 与只读 MCP 均需明确开启。       |

## 隐私优先

- 未同时取得用户同意和系统权限前，不启动采集。
- 应用、窗口、网站和浏览器隐私模式排除规则在落盘前执行。
- 原始事件默认保留 48 小时，也可缩短或选择完全不保留。
- 数据库使用 SQLCipher；随机密钥保存在 Keychain 或 Windows Credential Manager。
- 捕获文本始终是不可信数据，不会成为指令，也不会被执行。

完整说明见[隐私模型](docs/privacy.md)与[架构文档](docs/architecture.md)。

## 开发

需要 Node.js 24、Rust 1.89，以及 [Tauri 2 系统依赖](https://tauri.app/start/prerequisites/)。

```sh
git clone https://github.com/wellorbetter/open-history.git
cd open-history
npm ci
make check
npm run tauri dev
```

只查看浏览器里的界面样例时，运行 `npm run dev`，再打开 `?surface=compact`、
`?surface=history` 或 `?surface=setup`。

实现进度由一个 [OpenSpec change](openspec/changes/build-open-history-desktop/) 跟踪。修改采集或
隐私边界前，请先阅读[本地开发文档](docs/development.md)与[贡献指南](CONTRIBUTING.md)。

## 项目状态

OpenHistory 正在积极开发。GitHub Actions 会在 macOS 与 Windows 上检查前端和 Rust 工作区，
并生成未签名开发二进制。只有完整隐私与端到端验收通过后，才会发布签名安装包和稳定版本。
