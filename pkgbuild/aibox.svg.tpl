<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">
  <title>AIBox</title>
  <desc>A rounded container frame holding a minimal terminal prompt.</desc>
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="{{bg_top}}"/>
      <stop offset="1" stop-color="{{bg_bottom}}"/>
    </linearGradient>
    <linearGradient id="squircleRim" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="{{rim_top}}"/>
      <stop offset="1" stop-color="{{rim_bottom}}"/>
    </linearGradient>
    <linearGradient id="frame" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="{{frame_a}}"/>
      <stop offset="0.55" stop-color="{{frame_b}}"/>
      <stop offset="1" stop-color="{{frame_c}}"/>
    </linearGradient>
    <linearGradient id="chevron" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="{{chevron_a}}"/>
      <stop offset="1" stop-color="{{chevron_b}}"/>
    </linearGradient>
  </defs>

  <!-- App background squircle -->
  <rect x="16" y="16" width="480" height="480" rx="112" fill="url(#bg)" stroke="url(#squircleRim)" stroke-width="3"/>

  <!-- Hard shadow + isolated container box -->
  <rect x="92" y="132" width="328" height="260" rx="44" fill="{{box_shadow}}"/>
  <rect x="92" y="124" width="328" height="260" rx="44" fill="{{box_fill}}" stroke="url(#frame)" stroke-width="10"/>

  <!-- Terminal prompt >_ -->
  <path d="M170 212 L228 254 L170 296" fill="none" stroke="url(#chevron)" stroke-width="26" stroke-linecap="round" stroke-linejoin="round"/>
  <line x1="258" y1="296" x2="344" y2="296" stroke="{{cursor}}" stroke-width="26" stroke-linecap="round"/>
</svg>
