import { test, expect } from "@playwright/test";
import type { Page } from "@playwright/test";

const installTauriMocks = async (
  page: Page,
  options: { osType?: "macos" | "windows" | "linux" } = {},
) => {
  const osType = options.osType ?? "macos";
  await page.addInitScript((mockedOsType) => {
    const createModel = (id: string, downloaded: boolean) => ({
      id,
      name: id
        .split("-")
        .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
        .join(" "),
      description: "Speech model",
      filename: `${id}.bin`,
      url: null,
      sha256: null,
      size_mb: 512,
      is_downloaded: downloaded,
      is_downloading: false,
      partial_size: 0,
      is_directory: false,
      engine_type: "whisper",
      accuracy_score: 4,
      speed_score: 4,
      supports_translation: true,
      is_recommended: false,
      supported_languages: ["en", "zh"],
      supports_language_selection: true,
      is_custom: false,
    });

    const bindings = {
      transcribe: {
        id: "transcribe",
        name: "Transcribe Shortcut",
        description: "Start or stop recording",
        default_binding: "command+shift+r",
        current_binding: "command+shift+r",
      },
      cancel: {
        id: "cancel",
        name: "Cancel Shortcut",
        description: "Cancel active recording",
        default_binding: "escape",
        current_binding: "escape",
      },
    };

    const settings = {
      bindings,
      push_to_talk: false,
      audio_feedback: true,
      audio_feedback_volume: 0.7,
      sound_theme: "default",
      selected_model: "whisper-large-v3",
      asr_provider_id: "built_in_local",
      asr_providers: [
        {
          id: "built_in_local",
          label: "Built-in local models",
          base_url: "",
          kind: "built_in_local",
        },
      ],
      asr_api_keys: {},
      asr_models: {},
      transcription_profiles: [
        {
          id: "local_default",
          name: "Local default",
          description: "Local model, automatic language detection.",
          asr_provider_id: "built_in_local",
          asr_model: null,
          language: "auto",
          translate_to_english: false,
          post_process_enabled: false,
          post_process_preset_id: null,
          is_builtin: true,
        },
      ],
      selected_transcription_profile_id: "local_default",
      selected_microphone: "Default",
      selected_output_device: "Default",
      translate_to_english: false,
      selected_language: "auto",
      debug_mode: false,
      history_limit: 500,
      post_process_enabled: false,
      post_process_providers: [],
      post_process_api_keys: {},
      post_process_models: {},
      post_process_reasoning_efforts: {},
      post_process_prompts: [],
      post_process_presets: [],
      mute_while_recording: true,
      append_trailing_space: true,
      app_language: "en",
      experimental_enabled: true,
      context_awareness_enabled: false,
      keyboard_implementation: "tauri",
      show_tray_icon: true,
      paste_delay_ms: 100,
      typing_tool: "auto",
      external_script_path: null,
      custom_filler_words: [],
      whisper_accelerator: "auto",
      ort_accelerator: "auto",
      whisper_gpu_device: 0,
      extra_recording_buffer_ms: 0,
    };

    const models = [
      "whisper-large-v3",
      "whisper-medium",
      "whisper-small",
      "faster-whisper-large-v3",
      "parakeet-v2",
      "whisper-turbo",
    ].map((id, index) => createModel(id, index === 0));
    let contextProbeRuns = [
      {
        id: 1,
        history_entry_id: null,
        captured_at: 1730000000,
        source: "settings_debug",
        status: "success",
        confidence: "high",
        latency_ms: 18,
        app_name: "TextEdit",
        bundle_id: "com.apple.TextEdit",
        pid: 123,
        window_title: "Untitled",
        element_role: "AXTextArea",
        element_subrole: null,
        is_secure: false,
        value_text: "hello world",
        before_text: "hello ",
        selected_text: "world",
        after_text: "",
        selected_location_utf16: 6,
        selected_length_utf16: 5,
        number_of_characters: 11,
        available_attributes_json: '["AXValue"]',
        failure_reason: null,
        truncated: false,
      },
    ];

    let callbackId = 1;
    (window as any).__TAURI_OS_PLUGIN_INTERNALS__ = {
      platform: mockedOsType,
      os_type: mockedOsType,
      arch: mockedOsType === "macos" ? "aarch64" : "x86_64",
      family: mockedOsType === "windows" ? "windows" : "unix",
      version: "14",
      eol: "\n",
      exe_extension: "",
    };
    (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: () => undefined,
    };
    (window as any).__TAURI_INTERNALS__ = {
      transformCallback: () => callbackId++,
      unregisterCallback: () => undefined,
      invoke: async (cmd: string, args?: { providerId?: string }) => {
        if (cmd === "plugin:event|listen") return callbackId++;
        if (cmd === "plugin:event|unlisten") return null;
        if (cmd === "plugin:os|locale") return "en-US";
        if (cmd === "plugin:app|version") return "0.1.2";
        if (cmd.includes("macos-permissions")) return true;

        switch (cmd) {
          case "has_any_models_available":
            return true;
          case "get_available_models":
            return models;
          case "get_current_model":
          case "get_transcription_model_status":
            return "whisper-large-v3";
          case "get_app_settings":
          case "get_default_settings":
            return settings;
          case "get_available_microphones":
          case "get_available_output_devices":
            return [{ index: "default", name: "Default", is_default: true }];
          case "check_custom_sounds":
            return { start: false, stop: false };
          case "initialize_enigo":
          case "initialize_shortcuts":
          case "change_context_awareness_enabled_setting":
            return null;
          case "get_context_probe_runs":
            return contextProbeRuns;
          case "capture_focused_context_after_delay":
          case "capture_focused_context": {
            const nextRun = {
              ...contextProbeRuns[0],
              id: contextProbeRuns.length + 1,
              value_text: "captured text",
              before_text: "captured ",
              selected_text: "text",
              selected_location_utf16: 9,
              selected_length_utf16: 4,
            };
            contextProbeRuns = [nextRun, ...contextProbeRuns];
            return nextRun;
          }
          case "clear_context_probe_runs":
            contextProbeRuns = [];
            return null;
          case "get_asr_provider_status":
            return {
              provider_id: args?.providerId ?? "built_in_local",
              configured: true,
              api_key_source: "none",
              model: "local",
              error: null,
            };
          default:
            return null;
        }
      },
    };
  }, osType);
};

test.describe("SpeakMore App", () => {
  test("dev server responds", async ({ page }) => {
    // Just verify the dev server is running and responds
    const response = await page.goto("/");
    expect(response?.status()).toBe(200);
  });

  test("page has html structure", async ({ page }) => {
    await page.goto("/");

    // Verify basic HTML structure exists
    const html = await page.content();
    expect(html).toContain("<html");
    expect(html).toContain("<body");
  });

  test("renders and scrolls at the minimum window size", async ({ page }) => {
    await page.setViewportSize({ width: 680, height: 570 });
    await installTauriMocks(page);
    await page.goto("/");

    await expect(page.getByRole("button", { name: /General/i })).toBeVisible();
    await expect(page.getByRole("button", { name: /Models/i })).toBeVisible();

    const metrics = await page.evaluate(() => {
      const scroll = document.querySelector(".overscroll-contain");
      if (!scroll) return null;
      scroll.scrollTop = 400;
      return {
        clientHeight: scroll.clientHeight,
        scrollHeight: scroll.scrollHeight,
        scrollTop: scroll.scrollTop,
        horizontalOverflow: document.documentElement.scrollWidth > innerWidth,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics?.scrollHeight).toBeGreaterThan(metrics?.clientHeight ?? 0);
    expect(metrics?.scrollTop).toBeGreaterThan(0);
    expect(metrics?.horizontalOverflow).toBe(false);
  });

  test("resets scroll when switching settings sections", async ({ page }) => {
    await page.setViewportSize({ width: 680, height: 570 });
    await installTauriMocks(page);
    await page.goto("/");

    await expect(page.getByRole("button", { name: /Models/i })).toBeVisible();
    await page.evaluate(() => {
      const scroll = document.querySelector(".overscroll-contain");
      if (scroll) scroll.scrollTop = 400;
    });
    await page.getByRole("button", { name: /Models/i }).click();

    await expect(
      page.getByRole("button", { name: /All Languages/i }),
    ).toBeVisible();

    const scrollTop = await page.evaluate(
      () => document.querySelector(".overscroll-contain")?.scrollTop ?? -1,
    );
    expect(scrollTop).toBe(0);
  });

  test("renders context probe controls in experimental macOS settings", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 680, height: 570 });
    await installTauriMocks(page);
    await page.goto("/");

    await page.getByRole("button", { name: /Advanced/i }).click();

    await expect(
      page.getByText("Focused Context for Post-processing"),
    ).toBeVisible();
    await expect(page.getByText("macOS only")).toBeVisible();
    await expect(
      page.getByRole("button", { name: /Capture Focused Context/i }),
    ).toBeVisible();
    await expect(page.getByText("TextEdit", { exact: true })).toBeVisible();

    await page
      .getByRole("button", { name: /Capture Focused Context/i })
      .click();
    await expect(page.getByText("captured text")).toBeVisible();

    await page.getByRole("button", { name: /Clear Probe History/i }).click();
    await expect(
      page.getByText("No context probes captured yet."),
    ).toBeVisible();
  });

  test("shows focused context as unavailable on non-macOS", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 680, height: 570 });
    await installTauriMocks(page, { osType: "windows" });
    await page.goto("/");

    await page.getByRole("button", { name: /Advanced/i }).click();

    await expect(
      page.getByText(
        "Focused context capture is experimental and currently unavailable on this operating system.",
      ),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: /Capture Focused Context/i }),
    ).toHaveCount(0);
  });
});
