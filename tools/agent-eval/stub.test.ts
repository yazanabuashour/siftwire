import { expect, test } from "bun:test"
import * as NodeFS from "node:fs"
import * as NodeOS from "node:os"
import * as NodePath from "node:path"
import * as NodeProcess from "node:process"

import { protocol, responseSchema } from "./contract"

test("independent executable stub returns real inspection receipts without Pi configuration", async () => {
  const root = NodeFS.mkdtempSync(
    NodePath.join(NodeOS.tmpdir(), "siftwire-stub-"),
  )
  try {
    const workspace = NodePath.join(root, "workspace")
    const bin = NodePath.join(root, "bin")
    const skill = NodePath.join(workspace, ".agents/skills/siftwire/SKILL.md")
    NodeFS.mkdirSync(NodePath.dirname(skill), { recursive: true })
    NodeFS.mkdirSync(bin)
    NodeFS.writeFileSync(skill, "Synthetic skill")
    NodeFS.writeFileSync(
      NodePath.join(bin, "siftwire"),
      '#!/bin/sh\n[ "$1" = config ] || exit 1\ncat >> "$HOME/inputs"\nprintf \'{"sources":[]}\\n\'\n',
      { mode: 0o700 },
    )
    const request = {
      protocol,
      workspace,
      skill_path: skill,
      artifact_dir: NodePath.join(root, "artifacts"),
      prompts: [
        "Run a normal SiftWire configuration inspection",
        "Run a normal SiftWire configuration inspection again",
      ],
      tool_env: { PATH: `${bin}:/usr/bin:/bin`, HOME: root },
    }
    NodeFS.symlinkSync(
      NodePath.join(import.meta.dir, "stub"),
      NodePath.join(bin, "inspection-adapter"),
    )
    const child = Bun.spawn(["inspection-adapter"], {
      stdin: new Blob([JSON.stringify(request)]),
      stdout: "pipe",
      stderr: "pipe",
      env: {
        ...NodeProcess.env,
        PATH: `${bin}:${NodeProcess.env["PATH"] ?? ""}`,
        PI_CODING_AGENT_DIR: NodePath.join(root, "no-pi"),
      },
    })
    try {
      const [code, stdout, stderr] = await Promise.all([
        child.exited,
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
      ])
      expect(stderr).toBe("")
      expect(code).toBe(0)
      const result = responseSchema.parse(JSON.parse(stdout))
      expect(result.runtime).toEqual({
        adapter: "deterministic-inspection-stub",
        model: null,
        reasoning_effort: null,
      })
      expect(
        responseSchema.safeParse({
          ...result,
          runtime: { ...result.runtime, model: " \n" },
        }).success,
      ).toBe(false)
      expect(result.turns).toEqual(
        request.prompts.map(() => ({
          final_message: '{"sources":[]}\n',
          assistant_calls: 0,
          actions: [
            { kind: "read", path: skill },
            { kind: "command", command: "siftwire config" },
          ],
        })),
      )
      expect(NodeFS.readFileSync(NodePath.join(root, "inputs"), "utf8")).toBe(
        '{"action":"inspect_config"}\n'.repeat(2),
      )
      expect(NodeFS.existsSync(NodePath.join(root, "no-pi"))).toBe(false)
    } finally {
      child.kill()
      await child.exited
    }
  } finally {
    NodeFS.rmSync(root, { recursive: true, force: true })
  }
})
