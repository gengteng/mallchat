![](./doc/logo-256x256.png)

# MallChat Rust 后端实现

[WIP] 该项目处于开发中，功能不完整，欢迎贡献代码。

详细介绍见 Java 实现的后端 [MallChat](https://github.com/zongzibinbin/MallChat)

## 开发

* 本项目支持 **独立部署**、**[shuttle](https://www.shuttle.rs/) 云端部署**，后续将进一步支持 Docker 部署。

### 本地启动

```shell
# 拉取代码
git clone https://github.com/gengteng/mallchat
cd mallchat

# 编译，生产发布需要加上 `--release`
cargo build --no-default-features

# 将配置文件拷贝到目标目录下
cp server.example.toml server.toml

# 修改 server.toml 中的配置
# vi server.toml or use an editor

# 启动
cargo run --no-default-features
# 或者 ./target/debug/mallchat 启动
# 只要确保启动所在的当前目录有正确的 server.toml 即可

# 浏览器打开 http://localhost:8080/
```

### shuttle.rs 部署

[shuttle](https://www.shuttle.rs/) 的具体使用方法可在其主页点击 `Start Building` 按钮查看，需要使用 GitHub 登录。

> 假设你要部署到 `https://$YOUR_PROJECT_NAME.shuttleapp.rs/` 。

```shell
# 拉取代码
git clone https://github.com/gengteng/mallchat
cd mallchat

# 创建 shuttle.rs 的配置文件
cp Secrets.example.toml Secrets.toml

# 修改 Secrets.toml 中的配置
# vi Secrets.toml or use an editor

# 启动你自己的项目
cargo shuttle project start --name $YOUR_PROJECT_NAME

# 部署
cargo shuttle deploy --name $YOUR_PROJECT_NAME

# 浏览器打开 https://$YOUR_PROJECT_NAME.shuttleapp.rs/
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
|   shuttle    | 云部署平台           | [https://www.shuttle.rs](https://www.shuttle.rs)                     |

## 协议

Apache-2.0