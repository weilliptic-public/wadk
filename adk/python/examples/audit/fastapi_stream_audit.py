#!/usr/bin/env python3

from __future__ import annotations

import os
import sys
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Optional

from fastapi import FastAPI, HTTPException, Request
from pydantic import BaseModel, Field

# Allow importing local packages when run from the repository root or examples/.
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

import weil_ai


identity = "weil_xxx"

agent = weil_ai.WeilAgent(
    None,
    identity=identity,
    sentinel_host="https://sentinel.weilliptic.ai",
    verify=True,
)


class OrderIn(BaseModel):
    sku: str = Field(min_length=1)
    quantity: int = Field(gt=0)
    notes: Optional[str] = None


class QuoteOut(BaseModel):
    sku: str
    quantity: int
    estimated_total: float


#@agent.stream_audit("audits")
async def calculate_quote(sku: str, quantity: int) -> QuoteOut:
    price = 19.99 if sku.lower() == "notebook" else 9.99
    return QuoteOut(
        sku=sku,
        quantity=quantity,
        estimated_total=round(price * quantity, 2),
    )


@asynccontextmanager
async def lifespan(app: FastAPI):
    try:
        yield
    finally:
        await agent.close()


app = FastAPI(title="Weil Stream Audit FastAPI Example", lifespan=lifespan)


@app.get("/health")
async def health():
    return {"ok": True}


@app.post("/orders")
@agent.stream_audit()
async def create_order(order: OrderIn, request: Request):
    quote = await calculate_quote(order.sku, order.quantity)
    return {
        "accepted": True,
        "order": order,
        "quote": quote,
        "path": request.url.path,
    }


@app.get("/quote", response_model=QuoteOut)
@agent.stream_audit()
async def quote(sku: str, quantity: int):
    if quantity <= 0:
        raise HTTPException(status_code=400, detail="quantity must be positive")
    return await calculate_quote(sku, quantity)
