"""Compile and execute the current Unicode regressions as a real 32-bit process.

Maintainer tool: requires an installed i686 Rust target and 32-bit C runtime.
"""
from pathlib import Path
import argparse, os, re, subprocess, tempfile

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rustc',default='rustc')
parser.add_argument('--linker',required=True,type=Path)
parser.add_argument('--runtime-libraries',required=True,type=Path)
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
source=(project/'tests/game_arkanoo.rs').read_text()
names=[
'ascii_viewport_layout_keeps_visible_kerning_ligatures_and_pixels_without_full_text_storage',
'unicode_cluster_lookahead_preserves_eager_normalization_and_variation_behavior',
'unicode_base_viewport_preserves_full_layout_pixels_and_utf8_segments',
'unicode_viewport_matches_full_rasters_across_the_pinned_scalar_tables',
'composed_cluster_viewport_keeps_pixels_and_split_mark_boundaries',
'viewport_compositions_match_full_font_rasters_for_pairs_and_chains',
'selector_viewport_preserves_raw_normalization_ligatures_and_exact_pixels',
'selector_run_compaction_keeps_normalization_metrics_pixels_and_segment_boundaries',
'mixed_selector_runs_preserve_original_shaping_and_pixels',
'mixed_selector_compaction_keeps_exact_allocation_and_final_starter',
'unconsumed_cluster_viewport_preserves_original_pixels',
'cluster_viewport_matches_every_pinned_combining_class_and_segment_boundary',
'selector_mark_cluster_viewport_retains_raw_mode_and_exact_pixels',
'giant_missing_mark_run_preserves_full_shaping_pixels_and_saved_source',
'mixed_giant_missing_mark_classes_preserve_original_rasters_and_boundaries',
'reordered_giant_missing_mark_runs_preserve_original_pixels_and_source',
'mixed_class_fallback_runs_match_all_pinned_class_pairs',
'late_composing_mark_viewports_preserve_full_shaping_and_saved_source',
'pinned_supported_composition_graph_is_acyclic_and_has_two_transitions',
'every_missing_mark_ascii_composition_keeps_exact_viewport_pixels',
'composed_fallback_display_rejects_unproven_raw_marks_and_lost_vertical_extents',
]
# Select complete existing test sections; their bodies stay byte-identical.
sections={match.group(1):match.group(0) for match in re.finditer(r'^#\[test\]\nfn (\w+)\([^\n]*\).*?(?=^#\[test\]|\Z)',source,re.M|re.S)}
assert all(name in sections for name in names),'Current Unicode tests were not found'
with tempfile.TemporaryDirectory(prefix='gamepkg-unicode32-') as temporary:
    root=Path(temporary)
    harness=root/'unicode32.rs'
    modules=[('dynamic_font','ports/arkanoo/source/src/dynamic_font.rs'),('dynamic_font_data','ports/arkanoo/source/src/dynamic_font_data.rs'),('unicode_shape_eager_reference','tests/fixtures/arkanoo_unicode_shape_eager.rs')]
    harness.write_text('#[cfg(not(target_pointer_width="32"))]\ncompile_error!("This check requires a real 32-bit target");\n'+''.join(f'#[path="{project/path}"]\nmod {name};\n' for name,path in modules)+'\n'.join(sections[name] for name in names))
    binary=root/'unicode32'
    subprocess.run([args.rustc,'--edition=2021','--test','-O','--target','i686-unknown-linux-gnu','-C','linker='+str(args.linker.resolve()),'-A','dead_code',str(harness),'-o',str(binary)],check=True,timeout=120)
    environment=dict(os.environ,LD_LIBRARY_PATH=str(args.runtime_libraries.resolve()))
    subprocess.run([str(binary),'--nocapture'],check=True,env=environment,timeout=120)
print('Current Unicode viewport and mixed-selector regressions passed in a real 32-bit executable.')
