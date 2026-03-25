<template>
    <div class="space-y-2">
        <KeystrokeRecorder v-model="combo" @recording-start="onRecordingStart" @recording-end="onRecordingEnd" />

        <u-alert
            v-for="(issue, idx) in issues"
            :key="idx"
            :icon="issue.severity === 'error' ? 'i-lucide-circle-x' : 'i-lucide-alert-triangle'"
            :color="issue.severity === 'error' ? 'error' : 'warning'"
            variant="soft"
            :description="issue.message"
        />
    </div>
</template>

<script setup lang="ts">
    import KeystrokeRecorder from '~/shared/ui/keystroke-recorder/keystroke-recorder.vue';
    import type { KeyCombo } from '~/shared/ui/keystroke-recorder/model/types';
    import { usePlatformStore } from '~/entities/platform';
    import { validateHotkey } from '../model/hotkey-constraints';

    const props = withDefaults(
        defineProps<{
            modelValue?: KeyCombo | null;
        }>(),
        {
            modelValue: null,
        }
    );

    const emit = defineEmits<{
        'update:modelValue': [combo: KeyCombo];
        validation: [issues: ReturnType<typeof validateHotkey>];
    }>();

    const platformStore = usePlatformStore();

    const combo = computed({
        get: () => props.modelValue,
        set: (value: KeyCombo | null) => {
            if (value) emit('update:modelValue', value);
        },
    });

    const issues = computed(() => {
        if (!props.modelValue) return [];
        const platform = platformStore.currentNativePlatform as 'windows' | 'linux' | 'macos' | null;
        return validateHotkey(props.modelValue, platform);
    });

    watch(issues, value => emit('validation', value));

    // Temporarily unregister the current hotkey while recording to prevent interception
    async function onRecordingStart() {
        if (!platformStore.isNativePlatform) return;
        try {
            const { unregister } = await import('@tauri-apps/plugin-global-shortcut');
            const shortcutStr = formatForTauri(props.modelValue);
            await unregister(shortcutStr);
        } catch {
            // Not in Tauri context or shortcut not registered — ignore
        }
    }

    async function onRecordingEnd() {
        if (!platformStore.isNativePlatform) return;
        try {
            const { register } = await import('@tauri-apps/plugin-global-shortcut');
            const { getCurrentWindow } = await import('@tauri-apps/api/window');
            const appWindow = getCurrentWindow();
            // Re-register current hotkey (or default Ctrl+`)
            const current = props.modelValue;
            const shortcutStr = formatForTauri(current);
            await register(shortcutStr, event => {
                if (event.state === 'Pressed') {
                    appWindow.emit('hotkey-pressed');
                } else if (event.state === 'Released') {
                    appWindow.emit('hotkey-released');
                }
            });
        } catch (e) {
            console.warn('Failed to re-register global shortcut:', e);
        }
    }

    function codeToTauriKey(code: string): string {
        if (code.startsWith('Key')) return code.slice(3); // KeyA → A
        if (code.startsWith('Digit')) return code.slice(5); // Digit1 → 1
        if (code.startsWith('Numpad')) return `Num${code.slice(6)}`; // Numpad0 → Num0
        return code; // Backquote, F1, Space, ScrollLock, etc.
    }

    function formatForTauri(combo: KeyCombo | null): string {
        if (!combo) return 'Control+Backquote';
        const parts: string[] = [];
        if (combo.ctrl) parts.push('Control');
        if (combo.alt) parts.push('Alt');
        if (combo.shift) parts.push('Shift');
        if (combo.meta) parts.push('Super');
        parts.push(codeToTauriKey(combo.code));
        return parts.join('+');
    }
</script>
