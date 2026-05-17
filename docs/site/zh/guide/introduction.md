# 简介

**arthash** 是一族占位图哈希：一个很短的字节串（17 B 到 400 B），能解码回原图的一个可辨识预览。用法与 [blurhash](https://blurha.sh/)、[thumbhash](https://evanw.github.io/thumbhash/) 或 [sqip](https://github.com/axe312ger/sqip) 完全一样——把 hash 存在图片 URL 旁边，等真图加载时先把预览渲染出来。

核心用 Rust 写。Python 通过 PyO3 wheel，TypeScript 通过 wasm-bindgen 复用同一份 Rust 代码。任何一端产出的 hash 都能在另一端解码。

## 它能替代什么

| 你现在用               | 换成 arthash 的            | 主要收益                                                |
| ---------------------- | -------------------------- | ------------------------------------------------------- |
| blurhash / thumbhash   | `DCT` 模式                 | 同字节数 PSNR 高 0.4 dB；JS 端 encode 1.9× / decode 1.4× |
| sqip（primitive 部分） | `TRIANGLE` / `CIRCLE` 模式 | 体积 1/9 – 1/16；编码 50–67× 快；能在浏览器原生 wasm 跑 |

shape / `PIXEL` 模式还可以接收外部调色板，把颜色字段压成 4 bit，同时画面自然带上调色板的视觉风格（品牌色、复古、莫兰迪等）。

## 怎么选模式

| 目标                   | 选哪个                                                |
| ---------------------- | ----------------------------------------------------- |
| 极致小体积，模糊外观   | `DCT`（≤ 24 B）                                       |
| 锐利形状，输出 SVG     | `TRIANGLE` / `CIRCLE` / `RECT`（50 – 400 B，按 n 变） |
| 品牌一致 / 复古占位图  | 任意 shape 模式 + 调色板（颜色压到 4 bit）            |
| 像素画风的马赛克       | `PIXEL`（25 – 130 B）                                 |

playground 默认 `TRIANGLE n=64 / baseSize 512 / RGB-565`，是个比较合理的起点。对大小敏感就调低 `n`；要极致小就上 `DCT`。

## 与 thumbhash / sqip 的关系

**thumbhash**（Evan Wallace，2023）—— blurhash 之后的演化版本，同样用 DCT 编码模糊缩略图，编码更紧凑、~24 字节，纯 JS 实现。arthash 的 `DCT` 模式直接对标它。

**sqip**（Tobias Baldauf，2017）—— 一个 Node 插件框架，用得最多的是 `sqip-plugin-primitive`（调用 Go [`primitive`](https://github.com/fogleman/primitive)，爬山法叠加 N 个几何原语拟合原图），输出 SVG 字符串。典型用法是构建期生成、内联到 HTML 当 LQIP。arthash 的 shape 模式对标 primitive 这一支——但因为是 wasm，也能在请求期实时编码。

### 特性对比

| 特性                         |     arthash     |     thumbhash      |          sqip           |
| ---------------------------- | :-------------: | :----------------: | :---------------------: |
| DCT 模糊缩略图（17–24 B）    |        ✅        |         ✅          |            ❌            |
| 几何原语 SVG                 |     ✅ 5 种      |         ❌          |       ✅ 多种插件        |
| 像素马赛克                   |        ✅        |         ❌          |            ❌            |
| 外部调色板（颜色压到 4 bit） |        ✅        |         ❌          |            ❌            |
| Potrace 风格描边 SVG         |        ❌        |         ❌          | ✅ `sqip-plugin-potrace` |
| WebP 输出                    |        ❌        |         ❌          |     ✅ 部分插件支持      |
| 解码到任意尺寸               |        ✅        |   ⚠️ 默认 ~32 px    |      ✅（SVG 矢量）      |
| Web / 浏览器 wasm            |        ✅        |      ✅ 纯 JS       |   ❌（依赖 Go 子进程）   |
| Python 绑定                  |  ✅ PyO3 wheel   | ⚠️ 纯 Python 慢 80× |            ❌            |
| Rust crate                   |        ✅        |         ✅          |            ❌            |
| 部署形态                     | 请求期 / 构建期 |  请求期 / 构建期   |        仅构建期         |

arthash 当前**不覆盖** sqip 的 Potrace 描边模式（位图轮廓化 → SVG path），也没做 WebP / data-URI 输出。如果你的场景需要这些，sqip 仍然是更合适的选择。

## arthash 怎么省 bit

arthash 把每一个 bit 都花在图像信息上，没有任何 header 浪费。具体分四层：

1. **不要 header —— 两端共识的 Codec。** hash 字节流本身不自描述，不带 magic number、不带模式标签、不带 bit width。模式、形状数、量化位宽、调色板这些都由 `Codec` 同时配给 encode 和 decode。
2. **按位打包，最后一个字节零填充。** LSB-first，hash 长度由 codec 完全决定。
3. **DCT 模式——频域 + 感知空间双重压榨。** Oklab 量化、`AB_SCALE = 5`、带符号幂函数 compander、三角形高频掩码、每通道独立选 AC scale。
4. **Shape / PIXEL——几何与颜色精打细算。** 对数尺度半径量化、RGB-565 或调色板可选、离散 alpha 等级、π-对称 theta 加半步偏移。

字节格式定在 [`docs/SPEC.md`](https://github.com/Jannchie/arthash/blob/main/docs/SPEC.md)。

## 下一步

- [**安装**](./installation) —— 拿到对应语言的 SDK。
- [**基础用法**](./basic-usage) —— encode / decode / 渲染 SVG。
- [**模式与 Codec**](./modes) —— 按字节预算选模式。
