![](./doc/logo-256x256.png)

# MallChat Rust 后端实现

[![LANGUAGE](https://img.shields.io/badge/Language-Rust-dea584)](https://www.rust-lang.org/)
[![LICENSE](https://img.shields.io/badge/license-Apache-2)](https://github.com/gengteng/mallchat/blob/main/LICENSE)
![GitHub code size in bytes](https://img.shields.io/github/languages/code-size/gengteng/mallchat)
[![dependency status](https://deps.rs/repo/github/gengteng/mallchat/status.svg)](https://deps.rs/repo/github/gengteng/mallchat)
[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/gengteng/mallchat/.github/workflows/main.yml?branch=main)](https://github.com/gengteng/mallchat/actions/workflows/ci.yml)
[![Coverage Status](https://coveralls.io/repos/github/gengteng/mallchat/badge.svg?branch=main)](https://coveralls.io/github/gengteng/mallchat?branch=main)

[![Security Status](https://www.murphysec.com/platform3/v31/badge/1673713415187357696.svg)](https://www.murphysec.com/console/report/1673713414419800064/1673713415187357696)

----

[WIP] 该项目处于开发中，功能不完整，欢迎贡献代码。

详细介绍见 Java 实现的后端 [MallChat](https://github.com/zongzibinbin/MallChat)

## 开发

* 本项目支持 **独立部署**、Docker 部署。

### 本地启动

```shell
# 拉取代码
git clone https://github.com/gengteng/mallchat
cd mallchat

# 编译，生产发布需要加上 `--release`
cargo build

# 将样例配置文件拷贝为正式配置文件
cp server.example.toml server.toml

# 修改 server.toml 中的配置（尤其是微信公众平台的配置）
# vi server.toml or use an editor

# 启动
cargo run
# 或者 ./target/debug/mallchat 启动
# 只要确保启动所在的当前目录有正确的 server.toml 即可

# 浏览器打开 http://localhost:8080/
```

### Docker 部署

```shell
# 拉取代码
git clone https://github.com/gengteng/mallchat
cd mallchat

# 使用字节跳动的仓库镜像编译，使用 docker build -t mallchat -f CN.Dockerfile .
docker build -t mallchat .

# 将样例配置文件拷贝为正式配置文件
cp docker-compose.example.yml docker-compose.yml

# 修改 docker-compose.yml 中的配置（尤其是微信公众平台的配置）
# vi docker-compose.yml or use an editor

# 启动
docker-compose -f docker-compose.yml up -d --build

# 浏览器打开 http://localhost:8080/
```

## 前端

[MallChatWeb](https://github.com/Evansy/MallChatWeb)

## 技术选型

|      技术      | 说明              | 官网                                                                   |
|:------------:|-----------------|----------------------------------------------------------------------|
|     Axum     | Web 框架          | [https://github.com/tokio-rs/axum](https://github.com/tokio-rs/axum) |
|    Tokio     | 异步运行时           | [https://tokio.rs](https://tokio.rs)                                 |
|    config    | 配置管理            |                                                                      |
|    SeaORM    | ORM 框架          | [https://www.sea-ql.org/SeaORM/](https://www.sea-ql.org/SeaORM/)     |
| jsonwebtoken | JWT 库           |                                                                      |
|    serde     | 序列化/反序列化框架      | [https://serde.rs](https://serde.rs)                                 |
|    utoipa    | swagger-ui 生成框架 |                                                                      |
|  validator   | 合法性校验框架         |                                                                      |
|   reqwest    | HTTP 客户端        |                                                                      |
| parking_lot  | 高性能锁实现          |                                                                      |

## 协议

Apache-2.0