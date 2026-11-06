// Browser-only transport adapter. All synthesis and sequencing run in Rust/Wasm.
let context, processor, outputGain, initialization, latestSong, metronome = 0;
// Muted verification still renders the complete engine and measures its output.
// The zero-gain node is connected before any processor can reach the speakers.
const mutedVerification = new URLSearchParams(window.location.search).get('muted') === '1';
const encoder = new TextEncoder();
const decoder = new TextDecoder();
function report(error) {
  window.dispatchEvent(new CustomEvent('rb-error', {detail: String(error)}));
}
export async function startAudio(songJson) {
  latestSong = songJson;
  if (context) await context.resume();
  if (processor) { setSong(latestSong); return; }
  if (initialization) return initialization;
  context = new AudioContext({latencyHint: 'interactive'});
  outputGain = context.createGain();
  outputGain.gain.setValueAtTime(mutedVerification ? 0 : 1, context.currentTime);
  outputGain.connect(context.destination);
  // Resume within the user gesture, before network work.
  const resumed = context.resume();
  initialization = (async () => {
    await resumed;
    const response = await fetch('/engine.wasm');
    if (!response.ok) throw new Error(`Engine load failed (${response.status})`);
    const module = await WebAssembly.compile(await response.arrayBuffer());
    await context.audioWorklet.addModule('/processor.js');
    const node = new AudioWorkletNode(context, 'rb-rust', {
      numberOfInputs: 0, numberOfOutputs: 1, outputChannelCount: [2],
      processorOptions: {module, song: encoder.encode(latestSong)}
    });
    await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('Audio initialization timed out')), 15000);
      node.onprocessorerror = () => {clearTimeout(timeout); reject(new Error('Rust audio processor failed')); report('Rust audio processor failed');};
      node.port.onmessage = ({data}) => {
        if (data.type === 'ready') {clearTimeout(timeout); resolve();}
        else if (data.type === 'error') {clearTimeout(timeout); reject(new Error(data.message)); report(data.message);}
        else if (data.type === 'position') {
          if (data.controlsBytes) {
            data.controls = JSON.parse(decoder.decode(data.controlsBytes));
            delete data.controlsBytes;
          }
          data.outputMuted = outputGain.gain.value === 0;
          window.dispatchEvent(new CustomEvent('rb-position', {detail: data}));
        }
      };
    });
    processor = node;
    processor.connect(outputGain);
    setSong(latestSong);
    command(4, metronome);
  })().catch(async error => {
    processor?.disconnect(); processor = null;
    outputGain?.disconnect(); outputGain = null;
    await context?.close(); context = null;
    report(error); throw error;
  }).finally(() => {initialization = null;});
  return initialization;
}
export function setSong(json) {
  latestSong = json;
  if (processor) processor.port.postMessage({type: 'song', bytes: encoder.encode(json)});
}
export function command(command, value = 0, requestId = 0) {
  if (command === 4) metronome = value;
  if (processor) processor.port.postMessage({type: 'command', command, value, requestId});
}
export function recordCommand(value, requestId) { command(6, value, requestId); }
