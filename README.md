------------------------------
## Real-Time Parcel Tracking System
A high-performance, concurrent backend service for live courier tracking, built with Rust. This system handles high-frequency location updates through a stateful architecture using WebSockets, Redis, and PostgreSQL.
------------------------------
## 🏗️ Architecture Overview
The system is designed for low-latency data ingestion and reliable delivery of location events:

* Rust Backend: Leverages the Tokio runtime for asynchronous I/O and Axum/Tungstenite for WebSocket management.
* Redis Pub/Sub: Acts as the real-time event bus to broadcast location updates from couriers to customers instantly.
* Redis Cache: Stores "Last Known Position" with a 5-hour TTL to allow for session resumption during network drops.
* PostgreSQL: Serves as the source of truth for user authentication (hashed passwords) and permanent delivery logs.
* JWT Authentication: Stateless authorization for both REST endpoints and WebSocket upgrade handshakes.
  

------------------------------
## 🚀 Key Features

* Live Tracking: Real-time coordinate broadcasting with 2-second resolution.
* Fault Tolerance: Implemented Heartbeats (Ping/Pong) to detect "zombie" connections and Backpressure to protect the database during peak loads.
* Session Recovery: Drivers can reconnect within a 5-hour window without losing state, thanks to Redis-backed session management.
* Security: Argon2/Bcrypt password hashing and secure JWT verification for all protected routes.
* Scalability: Containerized with Docker and orchestrated via Kubernetes (Kind) for local cluster simulation.
  

------------------------------
## 🛠️ Tech Stack

* Language: [Rust](https://www.rust-lang.org/)
* Runtime: [Tokio](https://tokio.rs/)
* Database: PostgreSQL
* Caching/PubSub: Redis
* Containerization: Docker & Kubernetes (Kind)
* Auth: JWT (JSON Web Tokens)
  

------------------------------
## 🚦 Getting Started## Prerequisites

* Docker & Docker Compose
* Rust (1.70+)

## Installation

   1. Clone the repository:
   
   git clone https://github.comPrati-source/axum_api
   cd axum_api
   
   2. Run the environment using Docker Compose:
   
   docker-compose up --build
   
   3. Run the Rust linter to verify code quality:
   
   cargo clippy
   
   
------------------------------
## 🧪 Engineering Highlights## Handling Unreliable Networks
Mobile signals are patchy. To solve this, the system implements a 60-second Pong timeout. If a courier's device fails to respond within this window, the connection is dropped gracefully to save server resources, while the last coordinate remains available in Redis for customer visibility.
## Why Rust?
I chose Rust for this project to ensure memory safety and thread safety without the overhead of a Garbage Collector, making it ideal for the high-frequency writes required by a logistics platform.
------------------------------
## 👨‍💻 Author
Pramod S B
Backend Developer based in Bengaluru, India.
------------------------------

Would you like me to help you write a "Technical Challenges" section? This is where you explain exactly how you solved a specific bug. It’s the first thing a Senior Engineer looks for to see if you actually wrote the code!
