<template>
    <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24">
        <!-- Outer circle -->
        <circle cx="12" cy="12" r="11.5" fill="currentColor" />
        <!-- Heptagon + Token scaled to fit inside circle -->
        <g transform="translate(6 6) scale(0.5)">
            <!-- Heptagon (rotated 180°) -->
            <path
                fill="white"
                transform="rotate(180 12 12)"
                d="M11.114 1.438a2 2 0 0 1 1.772 0l7.077 3.495a2 2 0 0 1 1.066 1.359l1.767 7.937a2 2 0 0 1-.374 1.662l-4.926 6.337a2 2 0 0 1-1.579.772H8.083a2 2 0 0 1-1.579-.772L1.578 15.89a2 2 0 0 1-.374-1.662l1.767-7.937a2 2 0 0 1 1.066-1.359z"
            />
            <!-- Token (centered on heptagon visual centroid) -->
            <g transform="translate(1.8 2.5) scale(0.85)" fill="currentColor">
                <path
                    d="M9.196 9.866q.57-.691 1.297-1.028Q11.221 8.5 12 8.5q.798 0 1.516.338t1.288 1.028l5.165-2.864l-7.186-3.99q-.373-.218-.783-.218t-.783.218l-7.186 3.99zM11.5 21.142v-5.651q-1.3-.235-2.15-1.201T8.5 12q0-.333.054-.621t.167-.61L3.5 7.837v7.93q0 .454.227.82q.227.367.606.584zM12 14.5q1.037 0 1.768-.732T14.5 12t-.732-1.768T12 9.5t-1.768.732T9.5 12t.732 1.768T12 14.5m.5 6.642l7.167-3.97q.38-.218.606-.585q.227-.366.227-.82v-7.93l-5.221 2.932q.113.346.167.622T15.5 12q0 1.323-.85 2.29q-.85.966-2.15 1.2z"
                />
            </g>
        </g>
    </svg>
</template>

<script lang="ts">
    export default {
        name: 'CreoLogo',
    };
</script>
