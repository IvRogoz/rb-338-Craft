# Third-party notices

The app synthesizes audio at runtime and redraws its interface. The runtime uses
no original ReBirth recordings, sample banks, waveform ROM, image assets, or
sprites. Original local audio resources were analyzed offline only; procedural
parameters define the runtime oscillators, noise, and filter envelopes.

## Current software and dependency credits

The native interface uses eframe/egui and bundled fallback fonts. Their credits
are listed in [native/THIRD_PARTY_NOTICES.md](native/THIRD_PARTY_NOTICES.md).

## Current synthesis and offline evaluation

The acid synthesis uses an analytic oscillator, a pre-filter, a resonant filter
pair, and a DC blocker. Drum synthesis uses colored noise and damped-mode
profiles.

Source investigations, test fixtures, and reference comparisons used
[pingw33n/OpenRB-338](https://github.com/pingw33n/openrb-338/tree/main), pinned to
`cbca4492c0224dffaa477346bc8860559f3d853f`. Its study source is licensed under
GPL-3.0.

## Historical Open303 research

The previous engine adapted Open303's frequency-compensated filter coefficients
and resonance control mapping. Its fixed accented filter-envelope decay followed
Open303's documented stock 303 behavior. It was not a port of the entire engine.

Source: https://github.com/midilab/jc303/tree/main/src/dsp/open303

The MIT License

Copyright (c) 2009 Robin Schmidt (www.rs-met.com)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.

## Historical 808 cymbal research

The previous engine's synthesized 808 cymbal used published oscillator frequencies and filtering
measurements from Kurt James Werner, Jonathan S.
Abel and Julius O. Smith III, *The TR-808 Cymbal: a Physically-Informed,
Circuit-Bendable, Digital Model* (2014), distributed under Creative Commons
Attribution 3.0. That implementation generated oscillators and noise with
digital filters; it did not implement the paper's electrical component network.

Paper: https://www.icmc14-smc14.net/images/proceedings/OS24-B10-TheTR-808Cymbal.pdf

License: https://creativecommons.org/licenses/by/3.0/

## Reference-only original interface graphics

The numbered JPEGs under `.reference/standard-components/` are the original ReBirth
RB-338 Standard Components artwork by Propellerhead Software, extracted
statically from the archived ModPacker distribution for local visual reference.
The runtime interface is recreated with HTML/CSS and does not use these
images. No installer or original application is executed.
The artwork is separate from the Rust source and is not relicensed under the
implementation's open-source licenses. No original audio assets are included.

Source: https://archive.org/download/re-birth-rb-338-data/ReBirth%20RB-338/Install%20ReBirth%20ModPacker.exe

## Historical retired JavaScript solver

Historical notice: the retired JavaScript implementation's zero-delay coupled
diode ladder in `web/analog.js` adapted the topology and
solver from Timothy Place's [TapTools `diode_ladder.h`](https://github.com/tap/TapTools/blob/main/include/taptools/diode_ladder.h).

That solver is not part of the Rust runtime. Its license is retained with the
historical source attribution.

Copyright 2026 Timothy Place.

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
of the Software, and to permit persons to whom the Software is furnished to do
so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
