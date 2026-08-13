import { NextRequest, NextResponse } from "next/server";

export async function GET(request: NextRequest) {
  const { searchParams } = new URL(request.url);
  const email = searchParams.get("email") || "";
  const backendUrl = process.env.BACKEND_URL || "http://127.0.0.1:8000";

  const targetUrl = new URL(`${backendUrl}/auth/hackclub/login`);
  if (email) {
    targetUrl.searchParams.set("email", email);
  }

  return NextResponse.redirect(targetUrl.toString());
}

export async function POST(request: NextRequest) {
  return GET(request);
}
