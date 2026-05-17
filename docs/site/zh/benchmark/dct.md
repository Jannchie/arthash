# DCT vs thumbhash

arthash 的 `DCT` 模式直接对标 [thumbhash](https://evanw.github.io/thumbhash/) 和 [blurhash](https://blurha.sh/)：一个极小（~20 B）的模糊占位图。同字节预算，画质更高、编解码更快。

## encode @ 24 B 输出

### JS（baseline = thumbhash-js）

| 实现                | median |     vs baseline | 字节 |
| ------------------- | -----: | --------------: | ---: |
| arthash · ts (wasm) | 279 µs |     **1.9× 快** |   24 |
| thumbhash · JS      | 532 µs | 1.0× (baseline) |   24 |

### 原生（baseline = thumbhash-rust）

| 实现                      | median |     vs baseline | 字节 |
| ------------------------- | -----: | --------------: | ---: |
| arthash · Python (PyO3)   | 242 µs |    **1.27× 快** |   24 |
| arthash · Rust            | 243 µs |    **1.27× 快** |   24 |
| thumbhash · Rust (evanw)  | 308 µs | 1.0× (baseline) |   24 |
| thumbhash · Go (n16f.net) | 415 µs |        0.74× 慢 |   24 |
| thumbhash · Python (PyPI) |  25 ms |       0.012× 慢 |   24 |

arthash Rust 与 PyO3 速度持平（min ~228 µs / median ~243 µs）——PyO3 只多一层 GIL/PyBytes 封装，开销在 µs 量级，被批量测量噪声盖掉。

## decode @ DCT 24 B → RGBA

### JS（baseline = thumbhash-js 在它默认的 ~32 px 输出）

| 实现                | 输出尺寸 |     median |     vs baseline |
| ------------------- | -------: | ---------: | --------------: |
| arthash · ts (wasm) |   ~32 px |     116 µs |     **1.4× 快** |
| thumbhash · JS      |   ~32 px |     165 µs | 1.0× (baseline) |
| arthash · ts (wasm) |   256 px |    6.69 ms |  *(对方不支持)* |
| arthash · ts (wasm) |   512 px |   26.22 ms |  *(对方不支持)* |
| thumbhash · JS      |     256+ | API 不支持 |               — |

::: tip 为什么这在生产环境重要
thumbhash JS 解码 API 只输出 ~32 px，要变大只能 CSS 拉伸（拉糊）。arthash 直接 IDCT 到任意尺寸，省掉前端上采样这一步。
:::

### 原生 @ 256 px（baseline = thumbhash-go @ 256）

| 实现                    |  median |     vs baseline |
| ----------------------- | ------: | --------------: |
| arthash · Rust          | 2.06 ms |     **5.9× 快** |
| arthash · Python (PyO3) | 2.60 ms |     **4.7× 快** |
| thumbhash · Go @ 256    | 12.2 ms | 1.0× (baseline) |

thumbhash 的 Rust crate 在它自己的默认 ~32 px 输出下比 arthash 快；一旦要求显示尺寸缓冲（占位图实际场景），arthash 反超约 6×。

## arthash DCT 为什么更快

- **没有二段上采样。** arthash IDCT 直接落在目标尺寸，省掉 thumbhash 为了逃离 32 px 默认值要做的双线性上采样。
- **单遍 Oklab 反量化。** 没有独立的 sRGB → 线性阶段，IDCT 在感知空间里运行，最终的 sRGB clip 和输出写出融在一起。
- **必要时启用 wasm SIMD。** 打包的 wasm 通过 `wasm32-unknown-unknown` target flags 启用了 SIMD；现代浏览器和 Node 22 会自动走快路径。
