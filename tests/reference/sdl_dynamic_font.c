#include <SDL.h>
#include <SDL_ttf.h>
#include <ft2build.h>
#include FT_FREETYPE_H
#include <hb.h>
#include <hb-ft.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static void word(int n){unsigned int x=(unsigned int)n;unsigned char p[4]={x,x>>8,x>>16,x>>24};fwrite(p,1,4,stdout);}
int main(int argc,char**argv){if(argc<3||TTF_Init())return 1;TTF_Font*t=TTF_OpenFont(argv[1],26);if(!t)return 2;
if(!strcmp(argv[2],"text")){if(argc!=4)return 3;SDL_Surface*s=TTF_RenderUTF8_Blended(t,argv[3],(SDL_Color){200,200,200,255});if(!s)return 4;SDL_Surface*p=SDL_ConvertSurfaceFormat(s,SDL_PIXELFORMAT_RGBA32,0);word(p->w);word(p->h);for(int y=0;y<p->h;y++)fwrite((char*)p->pixels+y*p->pitch,4,p->w,stdout);SDL_FreeSurface(p);SDL_FreeSurface(s);return 0;}
FT_Library lib;FT_Face f;if(FT_Init_FreeType(&lib)||FT_New_Face(lib,argv[1],0,&f)||FT_Set_Char_Size(f,0,26*64,0,0))return 5;
if(!strcmp(argv[2],"glyphs")){word(f->num_glyphs);word(TTF_FontAscent(t));word(TTF_FontHeight(t));for(unsigned int i=0;i<(unsigned int)f->num_glyphs;i++){if(FT_Load_Glyph(f,i,FT_LOAD_DEFAULT|FT_LOAD_TARGET_NORMAL)||FT_Render_Glyph(f->glyph,FT_RENDER_MODE_NORMAL))return 6;FT_GlyphSlot g=f->glyph;word(g->bitmap_left);word(g->bitmap_top);word(g->bitmap.width);word(g->bitmap.rows);word(g->advance.x);for(unsigned int y=0;y<g->bitmap.rows;y++)fwrite(g->bitmap.buffer+y*g->bitmap.pitch,1,g->bitmap.width,stdout);}return 0;}
if(!strcmp(argv[2],"shape")){if(argc!=4)return 7;hb_font_t*h=hb_ft_font_create(f,NULL);hb_ft_font_set_load_flags(h,FT_LOAD_DEFAULT|FT_LOAD_TARGET_NORMAL);hb_buffer_t*b=hb_buffer_create();hb_buffer_set_direction(b,HB_DIRECTION_LTR);hb_buffer_set_script(b,HB_SCRIPT_UNKNOWN);hb_buffer_add_utf8(b,argv[3],-1,0,-1);hb_shape(h,b,NULL,0);unsigned int n;hb_glyph_info_t*info=hb_buffer_get_glyph_infos(b,&n);hb_glyph_position_t*pos=hb_buffer_get_glyph_positions(b,&n);word(n);int x=0,y=TTF_FontAscent(t)*64;for(unsigned int i=0;i<n;i++){word(info[i].codepoint);word(x+pos[i].x_offset);word(y-pos[i].y_offset);word(pos[i].x_advance);word(pos[i].y_advance);x+=pos[i].x_advance;y+=pos[i].y_advance;}hb_buffer_destroy(b);hb_font_destroy(h);return 0;}
return 8;}
