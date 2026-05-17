import { useEffect, useState } from "react";
import { listen } from "@objectiveai/sdk/viewer";

export function App() {
  // Placeholder: collect any `ping` events the host forwards through
  // the postMessage bridge so we can confirm the wiring works
  // end-to-end. Real `viewer_routes` will be declared in
  // `objectiveai.json` once the plugin has events to surface.
  const [events, setEvents] = useState<unknown[]>([]);

  useEffect(() => {
    const off = listen("ping", (value) => {
      setEvents((prev) => [...prev, value]);
    });
    return () => off();
  }, []);

  return (
    <main>
      <h1>psychological-operations</h1>
      <p>Plugin viewer scaffold. No routes wired up yet.</p>
      {events.length > 0 && (
        <pre>{JSON.stringify(events, null, 2)}</pre>
      )}
    </main>
  );
}
