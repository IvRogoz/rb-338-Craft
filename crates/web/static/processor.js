// AudioWorklet protocol bridge; DSP lives exclusively in engine.wasm.
class RustProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    this.failed = false;
    this.frames = 0;
    this.lastTick = -1;
    this.controlsRevision = -1;
    try {
      this.wasm = new WebAssembly.Instance(options.processorOptions.module, {}).exports;
      this.wasm.rb_init(sampleRate);
      this.song(options.processorOptions.song);
      this.port.onmessage = ({data}) => {
        if (this.failed) return;
        try {
          if (data.type === 'song') this.song(data.bytes);
          else if (data.type === 'command') {
            this.wasm.rb_command(data.command, data.value);
            if (data.command === 0 || data.command === 6) {
              this.publish({commandAck: data.command, commandValue: data.value, requestId: data.requestId});
            }
          }
        } catch (error) {this.fail(error);}
      };
      this.port.postMessage({type: 'ready'});
    } catch (error) {this.fail(error);}
  }
  fail(error) {
    this.failed = true;
    this.port.postMessage({type: 'error', message: String(error)});
  }
  song(bytes) {
    if (bytes.length > this.wasm.rb_input_capacity()) throw new Error('Song is too large');
    new Uint8Array(this.wasm.memory.buffer, this.wasm.rb_input_ptr(), bytes.length).set(bytes);
    if (this.wasm.rb_set_song(bytes.length) !== 0) throw new Error('Rust engine rejected song data');
  }
  publish(extra = {}) {
    const metrics = Array.from(new Float32Array(this.wasm.memory.buffer, this.wasm.rb_metrics_ptr(), 11));
    const data = {type:'position', position:this.wasm.rb_position(), tick:this.wasm.rb_tick(), playing:!!this.wasm.rb_playing(), metrics, ...extra};
    this.lastTick = data.tick;
    const revision = this.wasm.rb_controls_revision();
    if (revision !== this.controlsRevision) {
      const length = this.wasm.rb_controls_len();
      data.controlsBytes = new Uint8Array(this.wasm.memory.buffer, this.wasm.rb_controls_ptr(), length).slice();
      this.controlsRevision = revision;
    }
    this.port.postMessage(data);
  }
  process(_inputs, outputs) {
    const channels = outputs[0];
    if (this.failed) {for (const channel of channels) channel.fill(0); return true;}
    try {
      const frames = channels[0].length;
      // The ABI accepts at most 128 frames. Split larger future render quanta.
      let peak = 0;
      for (let offset = 0; offset < frames; offset += 128) {
        const count = Math.min(128, frames - offset);
        this.wasm.rb_render(count);
        const samples = new Float32Array(this.wasm.memory.buffer, this.wasm.rb_output_ptr(), count * 2);
        for (let i = 0; i < count; i++) {
          const left = samples[i * 2], right = samples[i * 2 + 1];
          if (!Number.isFinite(left) || !Number.isFinite(right)) throw new Error('Non-finite engine output');
          channels[0][offset + i] = left;
          if (channels[1]) channels[1][offset + i] = right;
          peak = Math.max(peak, Math.abs(left), Math.abs(right));
        }
      }
      this.frames += frames;
      if (this.frames >= sampleRate / 25 || this.lastTick !== this.wasm.rb_tick()) {
        this.frames = 0;
        this.publish({peak});
      }
    } catch (error) {for (const channel of channels) channel.fill(0); this.fail(error);}
    return true;
  }
}
registerProcessor('rb-rust', RustProcessor);
