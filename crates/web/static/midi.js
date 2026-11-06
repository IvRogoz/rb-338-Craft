// Web MIDI input bridge. Called only from an explicit user gesture; no audio is generated here.
let access = null;
let selectedId = null;
function forward(event) {
  window.dispatchEvent(new CustomEvent('rb-midi', {detail: {
    bytes: Array.from(event.data), timestamp: event.timeStamp,
    input: event.target.id
  }}));
}
function attach() {
  if (!access) return;
  for (const input of access.inputs.values()) input.onmidimessage =
    selectedId === null || input.id === selectedId ? forward : null;
  window.dispatchEvent(new CustomEvent('rb-midi-devices', {detail: midiInputs()}));
}
export async function requestMidi() {
  if (!navigator.requestMIDIAccess) throw new Error('Web MIDI is unavailable in this browser or context.');
  access = await navigator.requestMIDIAccess({sysex:false});
  access.onstatechange = attach;
  attach();
  return midiInputs();
}
export function midiInputs() {
  return access ? Array.from(access.inputs.values(), input => ({id:input.id, name:input.name || 'MIDI input', state:input.state})) : [];
}
export function selectMidiInput(id) { selectedId = id || null; attach(); }
export function closeMidi() {
  if (access) { for (const input of access.inputs.values()) input.onmidimessage = null; access.onstatechange = null; }
  access = null; selectedId = null;
}
