import ws from "k6/ws";
import { check, sleep } from "k6";

// 1. Configuration: Simulate 10 Drivers (Virtual Users)
export const options = {
  stages: [
    { duration: "210s", target: 5000 }, // Slowly ramp up to 5000 over 50 seconds
    { duration: "2m", target: 5000 }, // Stay at 5000 for 2 minutes
    { duration: "210s", target: 0 }, // Ramp down
  ], // Run simulation for 5 minutes
};

export default function () {
  // Replace with your actual Rust server URL

  const driverId = `driver_${__VU}`; // __VU is the unique ID (1 to 10)
  const parcelId = `1111${__VU}`;
  const url = `ws://host.docker.internal:8080/ws?parcel_id=${parcelId}&role=driver`;
  const params = {
    headers: {
      Authorization: `Bearer secret-token`,
    },
  };

  // 2. Initial position (e.g., Bengaluru area)
  let currentLat = Number((12.9716 + Math.random() * 0.01).toFixed(6));
  let currentLng = Number((77.5946 + Math.random() * 0.01).toFixed(6));

  const res = ws.connect(url, params, function (socket) {
    socket.on("open", function () {
      console.log(`${driverId} connected and picked up parcel.`);
      // 3. Set an interval to move and send data every 5 seconds
      const interval = socket.setInterval(function () {
        // Simulate movement (small random step)

        currentLat += (Math.random() - 0.5) * 0.001;
        currentLng += (Math.random() - 0.5) * 0.001;
        const timestampU64 = Date.now();
        const payload = JSON.stringify({
          parcel_id: parcelId.toString(),
          driver_id: driverId.toString(),
          latitude: currentLat,
          longitude: currentLng,
          timestamp: timestampU64,
          status: "picked_up".toString(),
        });

        socket.send(payload);
        console.log(
          `${driverId} updated location: ${currentLat.toFixed(4)}, ${currentLng.toFixed(4)}`,
        );
      }, 5000); // 5000ms = 5 seconds
    });
    socket.on("message", function (msg) {
      console.log(` received message: ${msg}`);
    });
    socket.on("error", (e) => {
      console.log(`Socket Error: ${e.error()}`);
    });
  });

  // Keep the connection open for the duration of the VU execution
  // Close after 5 minutes

  check(res, { "Connected successfully": (r) => r && r.status === 101 });
  // 4. Handle Pings from your Rust server to stay connected
  sleep(180);
}
