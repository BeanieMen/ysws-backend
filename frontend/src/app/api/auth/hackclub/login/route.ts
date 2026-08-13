import { NextRequest, NextResponse } from "next/server";

export async function GET(request: NextRequest) {
  const backendUrl = process.env.BACKEND_URL || "http://127.0.0.1:8000";
  const url = new URL(request.url);
  const targetUrl = `${backendUrl}/auth/hackclub/login${url.search}`;
  return NextResponse.redirect(targetUrl);
}
