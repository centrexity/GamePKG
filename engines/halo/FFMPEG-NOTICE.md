# Native movie decoder

The Linux profile builds FFmpeg 9.0.2 from its [official source archive](https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz), verified by SHA-256 `8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e`. The source tree includes FFmpeg's copyright notices and `COPYING.LGPLv2.1`.

This configuration uses LGPL-2.1-or-later components. It enables the Bink demuxer, Bink video decoder, RDFT/DCT audio decoders and the supporting image/audio utilities. GPL and nonfree components, network protocols, hardware autodetection, external libraries, assembly, command-line programs and encoders are disabled. The resulting static libraries are linked into the AGPL-3.0-or-later native OpenCE executable. The game bridge is provided under AGPL-3.0-or-later; the upstream FFmpeg files retain their own licenses.

The reviewed plan and maintainer exporters contain the exact source identities, generated configuration, compile commands and archive membership. Users build these sources locally; GamePKG retains the verified source tree. No proprietary Bink SDK, game video, audio recording, disc image or extracted game assets are included. The generated codec test clips contain only original black/white frames and silence.
