import { NextRequest, NextResponse } from "next/server";
import { backendFetch, proxyResponse } from "@/lib/backend";
import { config } from "dotenv";
config();

export async function GET(request: NextRequest) {
  const url = new URL(request.url);
  const pathname = url.pathname;
  const search = url.search;

  // If path starts with /api/user/auth/login or /auth/hackclub/login
  if (pathname.includes("auth/login") || pathname.includes("hackclub/login")) {
    const email = url.searchParams.get("email") || "";

    const targetUrl = process.env.APP_URL
      ? new URL(process.env.APP_URL)
      : new URL("/api/auth/hackclub/login", url.origin);
    if (email) targetUrl.searchParams.set("email", email);
    return NextResponse.redirect(targetUrl.toString());
  }

  // General fallback proxy for any unhandled /api/* or /auth/* request.
  // Static route handlers take precedence, but this keeps any future handler
  // on the same cookie-safe proxy path.
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
