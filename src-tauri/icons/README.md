# Icons

`tauri.conf.json` points at `32x32.png`, `128x128.png` and `icon.ico` in this
folder. They are not in the repository yet, so **`cargo tauri build` will fail
until they exist** (`cargo tauri dev` does not need them).

Generate the whole set from one square source image, 1024×1024 or larger:

```
pnpm tauri icon path\to\logo.png
```

That writes every size Windows, and later any other platform, expects.
