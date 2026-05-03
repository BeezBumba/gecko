<div align="center">

# Gecko

A WIP GameCube/Wii emulator and debugger written in Rust.

<img src="images/ffcccb.png" width="50%">

<img src="images/wario.png" width="30%"> <img src="images/sunshine.png" width="30%"> <img src="images/windwaker.png" width="30%">
<img src="images/luigi.png" width="30%"> <img src="images/re4.png" width="30%"> <img src="images/debugger.png" width="30%">

</div>

## Status

Gecko is still in development. Support may vary, while many games work perfectly (to my estimation at least), most will likely have varying degrees of visual glitches or be outright broken. Refer to the [screenshot database](https://emu.layle.dev/gecko/) to gauge compatiblity.

- PowerPC interpreter
- DSP LLE interpreter
- IPL skip patches for NTSC and PAL
- `wgpu` based renderer backend
- `wesl` based shader compiler
- LUA scripting/hooks system for runtime introspection
- A beautiful yet advanced egui-based debugging UI
- Symbol parsing from ELFs and IDA Pro databases
- RenderDoc captures with all sorts of debug markers
- ISO and RVZ support; also supports either packed as a ZIP
- [Support for web browser](https://gecko.layle.dev)
  - [incl. debugging capabilities](https://gecko.layle.dev/dbg)
- Terrible idle skipping :^)

Gecko currently does **not** implement sound and does **not** support Wii controls.

## Projects

| Crate       | Description                                                                                                                     |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `tinyapp`   | Lightweight emulator application with an egui/wgpu GUI, optional Lua scripting and idle-skip optimization                       |
| `debugger`  | Interactive GUI debugger built on egui with rendering support, hooks and scripting capabilities                                 |
| `web`       | WebAssembly build of the emulator for browser deployment via wasm-bindgen, with optional debug UI                               |
| `multitool` | CLI utility for analyzing, disassembling and extracting GC/Wii binaries/images (DOL, IPL, ISO/RVZ) with support for PPC and DSP |

## Building

```sh
cargo build -p multitool --release                               # multi-tool
cargo build -p tinyapp --release                                 # main application
cargo build -p debugger --release                                # debugger
wasm-pack build crates/web --target web --out-dir pkg --release  # web version
```

Certain features require certain feature flags such as `scripting` and `scripting-mut-traps`, however, the debugger has them all enabled (except `renderdoc-capture`).  
For more information refer to the GitHub CI actions file. PGO optimized compilation is supported, refer to the `Justfile`.

## Usage

```sh
multitool ipl --action decode ipl.encoded.bin ipl.decoded.bin
tinyapp --dol homebrew.dol  # may also require a DSP depending on the DOL
tinyapp --dvd game.iso --ipl ipl.decoded.bin --dsp dsp_rom.bin
```

The CLI options are largely the same across the sub projects (such as the debugger). For more options, see `--help`.

## Sister Projects
Gecko is being developed alongside other amazing emulators that shaped how Gecko came to be. Without them, Gecko wouldn't exist!

- [lazuli](https://github.com/vxpm/lazuli) authored by vxpm
- [solstice](https://codeberg.org/hazelwiss/solstice) authored by hazelwiss
- [beanwii](https://github.com/zaydlang/beanwii) authored by zayd

Besides these "sister projects", [Dolphin](https://github.com/dolphin-emu/dolphin) has also been a major contributor and the main reference for when things got tricky ;)