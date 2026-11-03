#!/usr/bin/env python3
"""Analytic unit tests for baselines.poly_seam_fitter and baselines.endpoint_pin.

Run: python scripts/test_seam_classical_baselines.py   (CPU, no pytest needed)
"""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

import torch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from baselines.endpoint_pin import endpoint_pin  # noqa: E402
from baselines.poly_seam_fitter import fit_poly_seam, poly_projection  # noqa: E402


def _poly_rows(b: int, n: int, w: int, degree: int, g: torch.Generator) -> torch.Tensor:
    """Rows whose head and tail windows are each an exact polynomial of degree."""
    x = torch.randn(b, n, dtype=torch.float64, generator=g)
    t = torch.arange(w, dtype=torch.float64)
    for sl in (slice(0, w), slice(n - w, n)):
        c = torch.randn(b, degree + 1, dtype=torch.float64, generator=g)
        x[:, sl] = sum(c[:, k : k + 1] * (t / w) ** k for k in range(degree + 1))
    return x


class PolySeamTests(unittest.TestCase):
    def setUp(self):
        self.g = torch.Generator().manual_seed(1234)

    def test_exact_polynomial_reconstructed(self):
        for d in (0, 1, 2, 3):
            x = _poly_rows(16, 256, 8, d, self.g)
            y = fit_poly_seam(x, degree=d, seam_w=8)
            torch.testing.assert_close(y, x, rtol=0, atol=1e-10)

    def test_lower_degree_poly_kept_by_higher_fit(self):
        x = _poly_rows(4, 64, 8, 1, self.g)
        torch.testing.assert_close(fit_poly_seam(x, degree=3, seam_w=8), x, rtol=0, atol=1e-10)

    def test_mid_untouched_and_shape_dtype(self):
        for dt in (torch.float32, torch.float64):
            x = torch.randn(5, 256, generator=self.g).to(dt)
            y = fit_poly_seam(x, degree=3, seam_w=8)
            self.assertEqual(y.shape, x.shape)
            self.assertEqual(y.dtype, dt)
            self.assertTrue(torch.equal(y[:, 8:-8], x[:, 8:-8]))

    def test_projection_idempotent_and_reduces_residual(self):
        x = torch.randn(32, 256, dtype=torch.float64, generator=self.g)
        y = fit_poly_seam(x, 3, 8)
        torch.testing.assert_close(fit_poly_seam(y, 3, 8), y, rtol=0, atol=1e-12)
        p = poly_projection(8, 3)
        torch.testing.assert_close(p @ p, p, rtol=0, atol=1e-12)
        torch.testing.assert_close(p, p.T, rtol=0, atol=1e-12)

    def test_batching_rowwise(self):
        x = torch.randn(7, 128, generator=self.g)
        y = fit_poly_seam(x, 3, 8)
        for i in range(7):
            self.assertTrue(torch.equal(fit_poly_seam(x[i : i + 1], 3, 8)[0], y[i]))

    def test_seam_w_edge_cases(self):
        x = torch.randn(3, 16, dtype=torch.float64, generator=self.g)
        # w = degree + 1: interpolation -> identity on window
        torch.testing.assert_close(fit_poly_seam(x, degree=3, seam_w=4), x, rtol=0, atol=1e-10)
        # w = 1, degree 0 -> identity
        torch.testing.assert_close(fit_poly_seam(x, degree=0, seam_w=1), x, rtol=0, atol=1e-12)
        # 2w == L allowed (no mid)
        self.assertEqual(fit_poly_seam(x, degree=3, seam_w=8).shape, x.shape)
        for bad in (dict(seam_w=0), dict(seam_w=9), dict(degree=4, seam_w=4), dict(degree=-1)):
            with self.assertRaises(ValueError):
                fit_poly_seam(x, **{"degree": 3, "seam_w": 8, **bad})
        with self.assertRaises(ValueError):
            fit_poly_seam(x[0], 3, 8)

    def test_dc_match(self):
        x = torch.randn(4, 64, dtype=torch.float64, generator=self.g)
        y = fit_poly_seam(x, 3, 8, dc_match=True)
        torch.testing.assert_close(y.mean(1), x.mean(1), rtol=0, atol=1e-12)

    def test_deterministic(self):
        x = torch.randn(9, 256, generator=self.g)
        self.assertTrue(torch.equal(fit_poly_seam(x, 3, 8), fit_poly_seam(x.clone(), 3, 8)))


class EndpointPinTests(unittest.TestCase):
    def setUp(self):
        self.g = torch.Generator().manual_seed(4321)

    def test_endpoints_pinned_exactly(self):
        for dt in (torch.float32, torch.float64):
            x = torch.randn(64, 256, generator=self.g).to(dt)
            ym = endpoint_pin(x, seam_w=8, mode="mean")
            self.assertTrue(torch.equal(ym[:, 0], ym[:, -1]))
            torch.testing.assert_close(ym[:, 0], ((x[:, 0].double() + x[:, -1].double()) / 2).to(dt))
            yz = endpoint_pin(x, seam_w=8, mode="zero")
            self.assertTrue(torch.all(yz[:, 0] == 0) and torch.all(yz[:, -1] == 0))

    def test_analytic_ramp(self):
        x = torch.zeros(1, 32, dtype=torch.float64)
        x[0, 0], x[0, -1] = 1.0, -1.0  # mean target 0
        y = endpoint_pin(x, seam_w=4, mode="mean")
        exp = torch.zeros(1, 32, dtype=torch.float64)
        exp[0, 0:4] = torch.tensor([0.0, -0.75, -0.5, -0.25])
        exp[0, 28:32] = torch.tensor([0.25, 0.5, 0.75, 0.0])
        torch.testing.assert_close(y, exp, rtol=0, atol=1e-12)

    def test_closed_cycle_mean_is_identity(self):
        x = torch.randn(8, 64, dtype=torch.float64, generator=self.g)
        x[:, -1] = x[:, 0]
        torch.testing.assert_close(endpoint_pin(x, 8, "mean"), x, rtol=0, atol=1e-12)

    def test_mid_untouched_shape_batching(self):
        x = torch.randn(6, 256, generator=self.g)
        y = endpoint_pin(x, 8, "mean")
        self.assertEqual(y.shape, x.shape)
        self.assertEqual(y.dtype, x.dtype)
        self.assertTrue(torch.equal(y[:, 8:-8], x[:, 8:-8]))
        for i in range(6):
            self.assertTrue(torch.equal(endpoint_pin(x[i : i + 1], 8, "mean")[0], y[i]))

    def test_seam_w_edge_cases(self):
        x = torch.randn(3, 16, dtype=torch.float64, generator=self.g)
        y = endpoint_pin(x, seam_w=1, mode="mean")  # only endpoints change
        self.assertTrue(torch.equal(y[:, 1:-1], x[:, 1:-1]))
        self.assertTrue(torch.equal(y[:, 0], y[:, -1]))
        self.assertEqual(endpoint_pin(x, seam_w=8).shape, x.shape)  # 2w == L
        for kw in (dict(seam_w=0), dict(seam_w=9), dict(mode="median")):
            with self.assertRaises(ValueError):
                endpoint_pin(x, **{"seam_w": 8, "mode": "mean", **kw})

    def test_deterministic(self):
        x = torch.randn(9, 256, generator=self.g)
        self.assertTrue(torch.equal(endpoint_pin(x, 8, "zero"), endpoint_pin(x.clone(), 8, "zero")))


if __name__ == "__main__":
    unittest.main(verbosity=2)
