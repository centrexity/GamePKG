/*
 * NTDLL error handling
 *
 * Copyright 2000 Alexandre Julliard
 * Copyright 2002 Andriy Palamarchuk
 * Copyright 2010 André Hentschel
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2.1 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this library; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301, USA
 */

ULONG WINAPI RtlNtStatusToDosErrorNoTeb( NTSTATUS status )
{
    ULONG ret;

    if (!status || (status & 0x20000000)) return status;

    /* 0xd... is equivalent to 0xc... */
    if ((status & 0xf0000000) == 0xd0000000) status &= ~0x10000000;

    /* now some special cases */
    if (HIWORD(status) == 0xc001 || HIWORD(status) == 0x8007 || HIWORD(status) == 0xc007)
        return LOWORD( status );

    ret = map_status( status );
    if (ret == ERROR_MR_MID_NOT_FOUND && status != STATUS_MESSAGE_NOT_FOUND)
        WARN( "no mapping for %08lx\n", status );
    return ret;
}
