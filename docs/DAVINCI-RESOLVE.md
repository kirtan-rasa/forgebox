# DaVinci Resolve export on Linux

ForgeBox includes a **DaVinci Resolve Linux** tool that creates QuickTime MOV files using Avid DNxHR video and uncompressed Linear PCM audio. The goal is an edit-friendly intermediate, not a small delivery encode.

## Requirements

- FFmpeg with the `dnxhd` encoder enabled (the FFmpeg encoder used for DNxHR).
- FFprobe, normally packaged with FFmpeg. ForgeBox uses it both to inspect the source and to verify the finished MOV.
- Enough free disk space for an intraframe intermediate. DNxHR files are substantially larger than typical H.264/H.265 delivery files.

The availability of the DNxHR encoder depends on the FFmpeg build installed on the system. ForgeBox reports a clear error if the required encoder is unavailable.

## Profiles

| Setting | Video format | Typical use |
|---|---|---|
| Automatic | Chooses HQ, HQX, or 444 from the source | Recommended default |
| DNxHR HQ | 8-bit 4:2:2 | General-purpose 8-bit sources |
| DNxHR HQX | 10-bit 4:2:2 | 10-bit YUV sources |
| DNxHR SQ | 8-bit 4:2:2 | Smaller editing intermediate |
| DNxHR LB | 8-bit 4:2:2 | Low-bitrate editing copy |
| DNxHR 444 | 10-bit 4:4:4 or RGB | 4:4:4 and RGB sources |

Automatic selection works as follows:

- 4:4:4 or RGB input selects DNxHR 444.
- More than 8-bit 4:2:0 or 4:2:2 input selects HQX.
- Other supported 8-bit input selects HQ.

Selecting a higher bit-depth profile does not restore information that is absent from the source. For example, converting 8-bit footage to HQX stores the result in a 10-bit container, but the original source still has 8-bit precision.

## Output details

- Container: QuickTime MOV.
- Video: selected DNxHR profile, source dimensions, and source frame timestamps passed through.
- Audio: every audio stream is mapped to 24-bit Linear PCM at 48 kHz.
- Metadata: global metadata and chapters are mapped when present; recognized color tags are copied.
- Performance: CPU encoding, limited to two FFmpeg threads.

ForgeBox checks the exported MOV with FFprobe. It verifies the DNxHR codec, selected profile, pixel format, and the expected audio format when the source contains audio.

## Warnings and limitations

The export is deliberately conservative about information it cannot preserve:

- Only the first video stream is exported. Additional video streams, such as alternate angles, are reported and omitted.
- Subtitle streams are reported but are not embedded in the output.
- Data streams and embedded attachments are reported but are not copied.
- Alpha transparency is not preserved by these DNxHR output settings.
- Interlaced sources are deinterlaced with BWDIF and exported as progressive video at the source frame rate. Field-rate motion detail is not retained.
- 4:4:4 or RGB sources are converted to 4:2:2 if a non-444 profile is explicitly selected.
- Sources above the output profile's bit depth lose some precision; ForgeBox reports this before encoding.
- HDR transfer tags are copied when recognized, but color management still needs to be configured correctly in the Resolve project.

The original input is not modified. Keep it as the source of truth, especially when the file contains subtitle, data, attachment, alpha, or alternate-video streams.

## Resolve compatibility

DNxHR in a MOV container is selected because it is an established editing intermediate and is listed by Blackmagic Design in its Resolve supported-codec reference. The reference distinguishes operating systems and editions, so consult the current table for the exact Resolve version and Linux configuration in use.

This ForgeBox feature does not install DaVinci Resolve, guarantee that every Linux distribution is officially supported by Blackmagic Design, or replace a test import on the target workstation.

## References

- [Blackmagic Design Support Center](https://www.blackmagicdesign.com/support) — see *DaVinci Resolve 21 Supported Codecs* for the current OS- and edition-specific codec matrix.
- [FFmpeg codecs documentation](https://ffmpeg.org/ffmpeg-codecs.html) — encoder and pixel-format support depends on the installed FFmpeg build.
