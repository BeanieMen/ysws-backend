import { NextRequest, NextResponse } from "next/server";
import { backendFetch, proxyResponse } from "@/lib/backend";

async function proxyBackendRequest(request: NextRequest) {
  const url = new URL(request.url);
  const pathname = url.pathname;
  const search = url.search;

  if (!pathname.startsWith("/api/") && !pathname.startsWith("/auth/")) {
    return new NextResponse("Not Found", { status: 404 });
  }

  const headers = new Headers();
  const contentType = request.headers.get("content-type");
  if (contentType) headers.set("content-type", contentType);

  // Pass the browser stream through unchanged so multipart uploads are not
  // buffered by Next.js before the Rust backend can enforce its streaming cap.
  const init: RequestInit & { duplex?: "half" } = {
    method: request.method,
    headers,
  };
  if (request.body) {
    init.body = request.body;
    init.duplex = "half";
  }

  return proxyResponse(await backendFetch(request, `${pathname}${search}`, init));
}

export async function GET(request: NextRequest) {
  return proxyBackendRequest(request);
}

export async function POST(request: NextRequest) {
  return proxyBackendRequest(request);
}

export async function PUT(request: NextRequest) {
  return proxyBackendRequest(request);
}

export async function DELETE(request: NextRequest) {
  return proxyBackendRequest(request);
}

export async function PATCH(request: NextRequest) {
  return proxyBackendRequest(request);
}
