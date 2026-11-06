These valid PNGs contain transparent RGBA pixels. Keeping them pre-generated
avoids encoding a 32 MiB image whenever the image-limit regression runs.

- `1024x512.png`: downscales to 64x32.
- `8193x1.png`: exceeds the 8192-pixel dimension limit.
- `4096x2048.png`: exceeds the 16 MiB decoded allocation limit.
