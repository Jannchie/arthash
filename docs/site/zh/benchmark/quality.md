# 画质对比

PSNR 相对原图、长边 256 px 输出、跨标准测试集平均。值越大越好；+3 dB ≈ 信噪比翻倍。

## 画质表（按 PSNR 降序）

| 输出                  |    字节 |    PSNR |
| --------------------- | ------: | ------: |
| sqip · 12 原语 SVG    | ~1100 B | 24.4 dB |
| arthash · DCT         |    17 B | 23.3 dB |
| thumbhash             |    17 B | 22.9 dB |
| arthash · TRIANGLE 12 |    77 B | 21.4 dB |
| arthash · CIRCLE 12   |    53 B | 20.7 dB |

## 关键结论

- **同 17 B 预算下**，arthash DCT 比 thumbhash 高 **+0.4 dB PSNR**——白送，无需调参。
- **arthash TRIANGLE 12** 用 77 B 拿到 21.4 dB。sqip 的 24.4 dB 更好，但字节是 **14×**（~1100 B），相当于用 1/14 字节换 3 dB 画质——做占位图通常是值得的折中。
- **arthash CIRCLE 12** 是 shape 模式中最小的（53 B），仍然能到 20.7 dB——与 3× 字节预算的 thumbhash 模糊图竞争，但用的是清晰的 SVG 原语而非糊的栅格。

## PSNR 重要与不重要的时候

PSNR 是有用的客观指标，但占位图本质是个感知问题。"乍一看是否像原图"的实际排名大致是：

1. **arthash TRIANGLE 24+** —— 形状可辨识，颜色真实。
2. **arthash DCT** —— 颜色与结构都强，模糊柔和。
3. **thumbhash** —— 与 arthash DCT 同思路，量化稍弱。
4. **arthash CIRCLE 12** —— 风格化强，保真度较低。
5. **sqip primitives** —— PSNR 高但体积大，只在字节充裕时有竞争力。

对于品牌一致的占位图，调色板模式的客观 PSNR 会更低，但主观感受更好——颜色分布与设计系统一致。

## 方法论

- **测试集**：24 张参考图（人像、风景、产品、UI 截图）。与 [JS 跨实现 bench](https://github.com/Jannchie/arthash/tree/main/bench/js-cross) 同一组。
- **解码尺寸**：长边 256 px。原生输出尺寸更小的实现（特别是 thumbhash 默认 ~32 px）在算 PSNR 前会被双线性上采样到 256。
- **色空间**：PSNR 在 sRGB 反 gamma 后计算。换成线性光或 Oklab 所有数值会整体上移 ~0.5 dB，但排名不变。
