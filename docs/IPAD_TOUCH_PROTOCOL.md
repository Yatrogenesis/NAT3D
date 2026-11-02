# iPad touch events

`SyncMessage::InputEvent` JSON accepts an optional `touch_id` integer (u32).
Each finger must keep a distinct ID from its down event through its up/end/cancel
event. Senders that omit this field keep the legacy single-contact ID 0.

Example message:

```json
{"InputEvent":{"event_type":"move","x":101.0,"y":100.0,"pressure":null,"tilt":null,"azimuth":null,"timestamp":0,"touch_id":7}}
```

Coordinates are viewport pixel positions. Pan is computed from the preceding
position of the same contact; the app applies the mouse-drag sensitivity of
0.01 to those deltas. Pinch factors and rotation angles are incremental between
samples. Adding or removing a contact resets the baseline without repeating
the last movement. Two-finger senders must provide distinct IDs to enable
pinch/rotate; messages without IDs remain single-touch.

The optional field is backward-compatible for JSON senders/receivers. Rust code
constructing the `InputEvent` enum variant must supply `touch_id: None` for
legacy events or `Some(id)` for identified contacts.

Local unit tests cover deltas, contact transitions, pinch/rotation, degenerate
positions and JSON compatibility. Real-device input and TCP transport still
require integration testing; this change does not certify that hardware path.
