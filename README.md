# test-instance (Rust Backend & Web App)

A high-performance Rust backend and Next.js web application built with **Hexagonal Architecture (Ports & Adapters)**, **NGINX Gateway Load Balancing**, **PostgreSQL**, **Redis**, and **Role-Based Access Control (RBAC)**.

---

## 🚀 How to Run the Whole Project

### Option 1: Run Full Stack with Docker Compose (Recommended)

```bash
# 1. Copy environment variables
cp .env.example .env

# 2. Start NGINX Gateway (Port 8000), PostgreSQL, Redis, and Backend
docker compose up -d

# 3. Start Frontend Web App (Port 3000)
cd frontend
npm install
npm run dev
```

- **Frontend App**: `http://localhost:3000`
- **Backend API Gateway (NGINX Load Balancer)**: `http://localhost:8000`

To scale backend worker instances dynamically behind the gateway:
```bash
docker compose up --scale backend=3 -d
```

---

### Option 2: Local Backend Development (Cargo)

```bash
# 1. Copy environment configuration
cp .env.example .env

# 2. Start PostgreSQL and Redis infrastructure
docker compose up postgres redis -d

# 3. Build & run Rust backend (Auto-applies migrations)
cargo run
```

Set `APP_URL` to the public frontend origin (for example,
`http://localhost:3001` during local development). Both OAuth callback URLs are
derived from it, so it must match the URL registered with Hack Club and
Hackatime.

---

## 🔐 User Roles (RBAC)

| Role | Description |
| --- | --- |
| `user` | Default role. Create projects, link Hackatime, upload banners, submit for review. |
| `reviewer` | All `user` rights + review shipped projects from `/dashboard`. |
| `admin` | Full control, including unshipped-project visibility and reviews, from `/admin`. |

---

## 📡 API Reference

### Authentication & Users
| Method | Endpoint | Description | Access |
| --- | --- | --- | --- |
| `GET` | `/auth/hackclub/login?email=…` | Starts Hack Club OAuth | Public |
| `GET` | `/auth/hackatime/login` | Starts Hackatime OAuth | Authenticated |
| `POST` | `/auth/logout` | Clears session cookie | Authenticated |
| `GET` | `/api/v1/me` | Returns current user profile & role | Authenticated |

### Projects & Banner Uploads
| Method | Endpoint | Description | Access |
| --- | --- | --- | --- |
| `GET` / `POST` | `/api/v1/projects` | List user projects / Create a project | Owner / Admin |
| `POST` | `/api/v1/projects/:id/banner` | Upload banner image (JPEG, PNG, WebP) | Owner / Admin |
| `POST` | `/api/v1/projects/:id/ship` | Ship a project for reviewer visibility | Owner |
| `PUT` | `/api/v1/projects/:id/hackatime-projects` | Link Hackatime projects | Owner / Admin |
| `GET` | `/api/v1/projects/:id/hackatime` | Fetch linked Hackatime time | Owner / Admin |
| `GET` | `/api/v1/projects/:id/lapses` | Fetch linked Lapse timelapses | Owner / Admin |

### Reviews & Admin
| Method | Endpoint | Description | Access |
| --- | --- | --- | --- |
| `GET` | `/api/v1/reviews/projects` | List shipped projects for reviewers; all projects for admins | Reviewer / Admin |
| `POST` | `/api/v1/projects/:id/reviews` | Submit project review | Reviewer / Admin |
| `GET` | `/api/v1/admin/users` | List all system users | Admin Only |
| `PUT` | `/api/v1/admin/users/:id/role` | Change user role (`user`, `reviewer`, `admin`) | Admin Only |
| `DELETE` | `/api/v1/admin/users/:id` | Delete user account | Admin Only |
| `DELETE` | `/api/v1/admin/projects/:id` | Delete project | Admin Only |

---

## 🧪 Testing

```bash
# Run unit tests
cargo test
```
