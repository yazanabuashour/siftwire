import { act } from "react"
import { expect, it } from "vitest"

import { configQuery } from "./config-query"
import {
  button,
  change,
  choose,
  click,
  client,
  configuration,
  host,
  input,
  pendingResponse,
  publisher,
  render,
  request,
  sentOutlets,
  setupConfigTest,
  settle,
  select,
} from "./config-test-utils"
import OutletsPage from "./OutletsPage"
import SettingsPage from "./SettingsPage"

setupConfigTest()

it("keeps publisher edits local, preserves notes on failure/refetch, and discards back to saved configuration", async () => {
  client.setQueryData(configQuery.queryKey, configuration())
  request.mockResolvedValue(
    Response.json(
      { error: { message: "Overlapping publisher matches" } },
      { status: 409 },
    ),
  )
  render(<OutletsPage />)
  choose("Rule for Example", "allow")
  act(() =>
    host.querySelector<HTMLInputElement>('input[type="checkbox"]')?.click(),
  )
  expect(host.querySelector("dialog")).toBeNull()
  expect(request).not.toHaveBeenCalled()
  click("Edit Example")
  change("Publisher name", "Renamed")
  click("Apply to draft")
  expect(request).not.toHaveBeenCalled()
  expect(host.querySelector("dialog")).toBeNull()
  click("Save publishers")
  await settle()
  expect(request).toHaveBeenCalledWith(
    "/api/v1/outlets",
    expect.objectContaining({
      method: "PUT",
      body: JSON.stringify({
        outlets: [
          { ...publisher, name: "Renamed", policy: "allow", enabled: false },
        ],
      }),
    }),
  )
  expect(host.textContent).toContain("Overlapping publisher matches")
  act(() =>
    client.setQueryData(configQuery.queryKey, {
      ...configuration(),
      outlets: [{ ...publisher, note: "Changed remotely" }],
    }),
  )
  await settle()
  click("Edit Renamed")
  expect(input("Note, optional").value).toBe(publisher.note)
  click("Close dialog")
  expect(select("Rule for Renamed").value).toBe("allow")
  click("Discard changes")
  expect(host.textContent).toContain("Publisher changes discarded.")
  click("Edit Example")
  expect(input("Note, optional").value).toBe("Changed remotely")
})

it("confirms and acknowledges an empty publisher collection despite a late config read", async () => {
  const write = pendingResponse()
  render(<OutletsPage />)
  click("Edit Example")
  click("Remove from draft")
  expect(request).not.toHaveBeenCalled()
  click("Save publishers")
  expect(request).not.toHaveBeenCalled()
  expect(host.textContent).toContain("Clear all publisher rules?")
  click("Clear all rules")
  await settle()
  expect(request).toHaveBeenCalledWith(
    "/api/v1/outlets",
    expect.objectContaining({
      method: "PUT",
      body: JSON.stringify({ outlets: [] }),
    }),
  )
  const lateRead = pendingResponse()
  act(() => {
    void client.refetchQueries(configQuery)
  })
  expect(lateRead.signal.aborted).toBe(false)
  // The runner omits empty collections, including outlets.
  write.respond(
    Response.json({
      runner_protocol: "siftwire-runner/v5",
      capabilities: ["current-news/v1"],
      rejected: false,
      summary: "Outlet policies replaced.",
    }),
  )
  await settle()
  expect(host.textContent).toContain("Publishers saved.")
  lateRead.respond(Response.json(configuration()))
  await settle()
  expect(client.getQueryData(configQuery.queryKey)).toEqual({
    ...configuration(),
    outlets: [],
  })
  expect(host.textContent).toContain("No publisher rules")
  expect(host.querySelector('[role="alert"]')).toBeNull()
  expect(lateRead.signal.aborted).toBe(true)
  expect(button("Save publishers").disabled).toBe(true)
  expect(button("Discard changes").disabled).toBe(true)
})

it("keeps settings drafts during refetch and errors, supports discard, and uses normalized returned options", async () => {
  request.mockResolvedValueOnce(
    Response.json({ rejected: true, summary: "Invalid time zone" }),
  )
  render(<SettingsPage />)
  change("Sports time zone", "GMT")
  act(() =>
    client.setQueryData(configQuery.queryKey, {
      ...configuration(),
      runtime_config: {
        ...configuration().runtime_config,
        sports_timezone: "Europe/London",
      },
    }),
  )
  await settle()
  expect(input("Sports time zone").value).toBe("GMT")
  click("Discard changes")
  expect(input("Sports time zone").value).toBe("Europe/London")
  change("Sports time zone", "UTC")
  click("Save settings")
  await settle()
  expect(host.textContent).toContain("Invalid time zone")
  expect(input("Sports time zone").value).toBe("UTC")
  const write = pendingResponse()
  click("Save settings")
  await settle()
  const lateRead = pendingResponse()
  act(() => {
    void client.refetchQueries(configQuery)
  })
  expect(lateRead.signal.aborted).toBe(false)
  write.respond(
    Response.json({
      runner_protocol: "siftwire-runner/v5",
      capabilities: ["current-news/v1"],
      runtime_config: { sports_timezone: "Etc/UTC" },
    }),
  )
  await settle()
  expect(input("Sports time zone").value).toBe("Etc/UTC")
  lateRead.respond(Response.json(configuration()))
  await settle()
  expect(input("Sports time zone").value).toBe("Etc/UTC")
  expect(client.getQueryData(configQuery.queryKey)).toEqual({
    ...configuration(),
    runtime_config: {
      ...configuration().runtime_config,
      sports_timezone: "Etc/UTC",
    },
  })
  expect(lateRead.signal.aborted).toBe(true)
  expect(button("Save settings").disabled).toBe(true)
  expect(button("Discard changes").disabled).toBe(true)
})

it("cancels unapplied publisher edits without leaving a stale collection draft", async () => {
  render(<OutletsPage />)
  click("Edit Example")
  change("Publisher name", "Cancelled name")
  act(() =>
    client.setQueryData(configQuery.queryKey, {
      ...configuration(),
      outlets: [{ ...publisher, name: "Remote name" }],
    }),
  )
  await settle()
  click("Close dialog")
  expect(host.textContent).toContain("Remote name")
  expect(host.textContent).not.toContain("Cancelled name")
  expect(button("Save publishers").disabled).toBe(true)
  expect(button("Discard changes").disabled).toBe(true)
  expect(request).not.toHaveBeenCalled()
})

it("adds a local publisher, freezes the collection while editing, and accepts only normalized saved rows", async () => {
  render(<OutletsPage />)
  expect(host.querySelector("dialog")).toBeNull()
  expect(select("Rule for Example").value).toBe("watch")
  click("Add publisher")
  change("Publisher name", "New publisher")
  change("Aliases", " NEW.TEST, Another name ")
  act(() =>
    client.setQueryData(configQuery.queryKey, {
      ...configuration(),
      outlets: [{ ...publisher, note: "Remote change" }],
    }),
  )
  await settle()
  click("Apply to draft")
  expect(request).not.toHaveBeenCalled()
  const added = {
    name: "New publisher",
    aliases: ["NEW.TEST", "Another name"],
    note: "",
    policy: "allow",
    enabled: true,
  }
  const normalized = {
    ...added,
    name: "Normalized publisher",
    aliases: ["new.test"],
  }
  const write = pendingResponse()
  click("Save publishers")
  await settle()
  expect(select("Rule for Example").disabled).toBe(true)
  expect(
    [
      ...host.querySelectorAll<HTMLInputElement>(".config-outlet-row input"),
    ].every((entry) => entry.disabled),
  ).toBe(true)
  write.respond(
    Response.json({
      runner_protocol: "siftwire-runner/v5",
      capabilities: ["current-news/v1"],
      outlets: [publisher, normalized],
    }),
  )
  await settle()
  expect(request).toHaveBeenCalledWith(
    "/api/v1/outlets",
    expect.objectContaining({ method: "PUT" }),
  )
  expect(sentOutlets()).toEqual([publisher, added])
  expect(host.textContent).toContain("Normalized publisher")
  expect(button("Save publishers").disabled).toBe(true)
  expect(client.getQueryData(configQuery.queryKey)).toEqual({
    ...configuration(),
    outlets: [publisher, normalized],
  })
})
