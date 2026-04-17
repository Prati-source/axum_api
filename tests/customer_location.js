import ws from "k6/ws";
import { check, sleep } from "k6";

// 1. Configuration: Simulate 10 Drivers (Virtual Users)
export const options = {
  vus: 2,
  duration: "5m", // Run simulation for 5 minutes
};

export default function () {
  // Replace with your actual Rust server URL

  // __VU is the unique ID (1 to 10)
  const parcelId = `123${__VU}`;
  const url = `ws://host.docker.internal:8080/customer?parcel_id=${parcelId}&role=customer`;
  const params = {
    headers: {
      Authorization: `Bearer secret-token`,
    },
  };

  const res = ws.connect(url, params, function (socket) {
    socket.on("open", function () {
      console.log(`Customer ${__VU} connected`);
      console.log("");
      // 3. Set an interval to move and send data every 5 seconds
      socket.setInterval(function () {
        // Simulate movement (small random step)
      }, 5000); // 5000ms = 5 seconds
    });
    socket.on("message", function (msg) {
      console.log(`Customer ${__VU} received: ${msg}`);
    });

    // 4. Handle Pings from your Rust server to stay connected

    // Keep the connection open for the duration of the VU execution
    socket.setTimeout(function () {
      socket.close();
    }, 300000); // Close after 5 minutes
  });

  check(res, { "Connected successfully": (r) => r && r.status === 101 });
}
