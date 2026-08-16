import { NextRequest, NextResponse } from "next/server";
import { backendFetch, proxyResponse } from "@/lib/backend";

export async function GET(request: NextRequest) {
  const url = new URL(request.url);
  const pathname = url.pathname;
  const search = url.search;

  // Proxy all /api/* and /auth/* requests to the Rust backend
  if (pathname.startsWith("/api/") || pathname.startsWith("/auth/")) {
    return proxyResponse(await backendFetch(request, `${pathname}${search}`));
  }

  return new NextResponse("Not Found", { status: 404 });
}

export async function POST(request: NextRequest) {
  return GET(request);
}

export async function PUT(request: NextRequest) {
  return GET(request);
}

export async function DELETE(request: NextRequest) {
  return GET(request);
}

export async function PATCH(request: NextRequest) {
  return GET(request);
}
