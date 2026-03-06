"""Tests for PresentPlanTool minimum-content validation."""

import tempfile
from pathlib import Path

from opendev.core.context_engineering.tools.implementations.present_plan_tool import PresentPlanTool


def test_rejects_trivially_short_plan():
    """Plan content under 100 chars should be rejected."""
    tool = PresentPlanTool()
    with tempfile.NamedTemporaryFile(mode="w", suffix=".md", delete=False) as f:
        f.write("Plan placeholder")
        f.flush()
        result = tool.execute(plan_file_path=f.name)

    assert result["success"] is False
    assert "too short" in result["error"]
    assert "16 chars" in result["error"]


def test_rejects_empty_plan():
    """Empty plan file should be rejected."""
    tool = PresentPlanTool()
    with tempfile.NamedTemporaryFile(mode="w", suffix=".md", delete=False) as f:
        f.write("")
        f.flush()
        result = tool.execute(plan_file_path=f.name)

    assert result["success"] is False
    assert "empty" in result["error"]


def test_accepts_real_plan():
    """A plan with substantial content (200+ chars) should pass validation."""
    tool = PresentPlanTool()
    real_plan = (
        "# Goal\n\n"
        "Refactor the authentication module to support OAuth2.\n\n"
        "## Steps\n\n"
        "1. Add OAuth2 provider configuration\n"
        "2. Implement token exchange flow\n"
        "3. Update session management to handle OAuth tokens\n"
        "4. Add unit tests for the new OAuth2 flow\n"
        "5. Update documentation\n"
    )
    assert len(real_plan.strip()) >= 100

    with tempfile.NamedTemporaryFile(mode="w", suffix=".md", delete=False) as f:
        f.write(real_plan)
        f.flush()
        # No ui_callback => auto-approve fallback path
        result = tool.execute(plan_file_path=f.name)

    assert result["success"] is True
    assert result.get("plan_approved") is True


def test_rejects_missing_file():
    """Non-existent plan file should be rejected."""
    tool = PresentPlanTool()
    result = tool.execute(plan_file_path="/tmp/nonexistent_plan_12345.md")
    assert result["success"] is False
    assert "not found" in result["error"]
