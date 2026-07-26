#!/usr/bin/env python3
"""Minimal Cycle-GAN Unet generator (Operational SelfONN) without pytorch_lightning.

Copied from upstream GAN_Arch_details.py (Blind-ECG-Restoration-by-Operational-Cycle-GANs)
with heavy training imports stripped so we can load model_weights_16NQ3.pth for wrap-R smoke.
"""
from __future__ import annotations

import sys
from pathlib import Path

import torch
from torch import nn

EXT = Path(__file__).resolve().parents[2] / "brand" / "artifacts" / "signal_heal_transfer" / "external"
CG = EXT / "Blind-ECG-Restoration-by-Operational-Cycle-GANs"
for p in (str(CG), str(CG / "Fastonn"), str(EXT / "fastonn")):
    if Path(p).is_dir() and p not in sys.path:
        sys.path.insert(0, p)

try:
    from Fastonn import SelfONN1d as SelfONN1dlayer  # noqa: E402
    from Fastonn import SelfONNTranspose1d as SelfONNTranspose1dlayer  # noqa: E402
except ModuleNotFoundError:
    # pip package is lowercase `fastonn`; upstream Cycle-GAN imports `Fastonn`.
    from fastonn import SelfONN1d as SelfONN1dlayer  # noqa: E402
    from fastonn import SelfONNTranspose1d as SelfONNTranspose1dlayer  # noqa: E402


class Upsample(nn.Module):
    def __init__(self, in_channels, out_channels, kernel_size=4, stride=2, padding=1, dropout=False):
        super().__init__()
        self.dropout = dropout
        self.block = nn.Sequential(
            SelfONNTranspose1dlayer(
                in_channels, out_channels, kernel_size, stride, padding, bias=nn.InstanceNorm1d, q=3
            ),
            nn.InstanceNorm1d(out_channels),
            nn.Tanh(),
        )
        self.dropout_layer = nn.Dropout(0.5)

    def forward(self, x, shortcut=None):
        x = self.block(x)
        if self.dropout:
            x = self.dropout_layer(x)
        if shortcut is not None:
            x = torch.cat([x, shortcut], dim=1)
        return x


class Downsample(nn.Module):
    def __init__(
        self, in_channels, out_channels, kernel_size=4, stride=2, padding=1, apply_instancenorm=False
    ):
        super().__init__()
        self.conv = SelfONN1dlayer(
            in_channels, out_channels, kernel_size, stride, padding, bias=nn.InstanceNorm1d, q=3
        )
        self.norm = nn.InstanceNorm1d(out_channels)
        self.relu = nn.Tanh()
        self.apply_norm = apply_instancenorm

    def forward(self, x):
        x = self.conv(x)
        if self.apply_norm:
            x = self.norm(x)
        return self.relu(x)


class CycleGAN_Unet_Generator(nn.Module):
    def __init__(self, filter=16):
        super().__init__()
        self.downsamples = nn.ModuleList(
            [
                Downsample(1, filter, kernel_size=5, padding=1, apply_instancenorm=False),
                Downsample(filter, filter * 2, kernel_size=5, padding=1),
                Downsample(filter * 2, filter * 4, kernel_size=5, padding=1),
                Downsample(filter * 4, filter * 8, kernel_size=5, padding=1),
                Downsample(filter * 8, filter * 8, kernel_size=5, padding=1),
            ]
        )
        self.upsamples = nn.ModuleList(
            [
                Upsample(filter * 8, filter * 8, kernel_size=5, padding=1),
                Upsample(filter * 16, filter * 4, dropout=False, kernel_size=5, padding=1),
                Upsample(filter * 8, filter * 2, dropout=False, kernel_size=5, padding=1),
                Upsample(filter * 4, filter, dropout=False, kernel_size=5, padding=1),
            ]
        )
        self.last = nn.Sequential(
            SelfONNTranspose1dlayer(filter * 2, 1, kernel_size=6, stride=2, padding=1, q=3),
            nn.Tanh(),
        )

    def forward(self, x):
        skips = []
        for layer in self.downsamples:
            x = layer(x)
            skips.append(x)
        skips = reversed(skips[:-1])
        for layer, s in zip(self.upsamples, skips):
            x = layer(x, s)
        return self.last(x)
