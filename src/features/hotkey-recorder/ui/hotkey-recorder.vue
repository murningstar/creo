<template>
    <div>
        <KeystrokeRecorder
            :model-value="modelValue"
            @update:model-value="onComboChange"
            @recording-start="onRecordingStart"
            @recording-end="onRecordingEnd"
            @cancelled="onCancelled"
        >
            <template #hint>
                <span v-if="validationError" class="text-error">{{ validationError }}</span>
                <span v-else>Click to change hotkey</span>
            </template>
        </KeystrokeRecorder>
    </div>
</template>

<script setup lang="ts">
    import { KeystrokeRecorder, type KeyCombo } from '~/shared/ui/keystroke-recorder';
    import { usePlatformStore } from '~/entities/platform';
    import { validateHotkey } from '../lib/hotkey-constraints';
    import { formatForTauri } from '../lib/format-for-tauri';

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
    }>();

    const platformStore = usePlatformStore();
    const validationError = ref<string | null>(null);

    function onComboChange(combo: KeyCombo) {
        const platform = platformStore.currentNativePlatform as 'windows' | 'linux' | 'macos' | null;
        const issues = validateHotkey(combo, platform);
        const error = issues.find(i => i.severity === 'error');

        if (error) {
            // Don't apply invalid combo, show error
            validationError.value = error.message;
            return;
        }

        // Valid combo — apply and clear error
        validationError.value = null;
        emit('update:modelValue', combo);
    }

    function onRecordingStart() {
        // Clear error when user starts new recording attempt
        validationError.value = null;

        if (!platformStore.isNativePlatform) return;
        // Temporarily unregister the current hotkey to prevent interception
        (async () => {
            try {
                const { unregister } = await import('@tauri-apps/plugin-global-shortcut');
                const shortcutStr = formatForTauri(props.modelValue);
                await unregister(shortcutStr);
            } catch {
                // Not in Tauri context or shortcut not registered
            }
        })();
    }

    function onCancelled() {
        // Escape pressed — keep current combo, no changes
    }

    async function onRecordingEnd() {
        if (!platformStore.isNativePlatform) return;
        try {
            const { register } = await import('@tauri-apps/plugin-global-shortcut');
            const { getCurrentWindow } = await import('@tauri-apps/api/window');
            const appWindow = getCurrentWindow();
            const shortcutStr = formatForTauri(props.modelValue);
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
</script>
