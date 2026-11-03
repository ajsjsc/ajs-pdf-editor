#!/usr/bin/env perl
# AJS fork: rename the product "PdfCraft" to "AJS PDF Editor" in user-visible text.
#
# Only the contents of Rust string literals ("…", b"…", r#"…"#) and the i18n catalogs (*.tsv) are
# changed; comments, identifiers (PdfCraftApp), folder/marker names (PdfCraftData,
# PdfCraftImageCrop) and the attribution to the upstream project stay as they are. Safe to run again
# after merging a new upstream release (idempotent).
#
# Usage (from the repository root): perl tools/ajs/rebrand.pl
use strict;
use warnings;
use File::Find;

my $NEW = 'AJS PDF Editor';
my @ROOTS = ('apps/pdfcraft', 'apps/pdfcraft-cli', 'crates');

# "PdfCraft" as a word, not part of an identifier, and not the attribution to the upstream project.
sub rebrand_text {
    my ($s) = @_;
    $s =~ s{(?<![A-Za-z0-9_])PdfCraft(?![A-Za-z0-9_])}{
        my ($pre, $post) = (substr($s, 0, $-[0]), substr($s, $+[0]));
        ($pre =~ /(?:Based on|version of|Fork of)\s*$/ || $post =~ /^\s*(?:source code|by the ArtCraft|contributors)/)
            ? 'PdfCraft' : $NEW
    }ge;
    return $s;
}

# Walk Rust source, rebranding only string literal contents.
sub rebrand_rust {
    my ($src) = @_;
    my $out = '';
    my $n = length $src;
    my $i = 0;
    while ($i < $n) {
        my $rest = substr($src, $i, 2);
        if ($rest eq '//') {                                   # line comment
            my $e = index($src, "\n", $i); $e = $n if $e < 0;
            $out .= substr($src, $i, $e - $i); $i = $e; next;
        }
        if ($rest eq '/*') {                                   # block comment (nested)
            my ($depth, $j) = (1, $i + 2);
            while ($j < $n && $depth) {
                my $t = substr($src, $j, 2);
                if ($t eq '/*') { $depth++; $j += 2 } elsif ($t eq '*/') { $depth--; $j += 2 } else { $j++ }
            }
            $out .= substr($src, $i, $j - $i); $i = $j; next;
        }
        if (substr($src, $i) =~ /\A(b?r(#*)")/) {              # raw string r#"…"#
            my ($open, $hashes) = ($1, $2);
            my $close = '"' . $hashes;
            my $start = $i + length $open;
            my $e = index($src, $close, $start); $e = $n if $e < 0;
            $out .= $open . rebrand_text(substr($src, $start, $e - $start)) . $close;
            $i = $e + length $close; next;
        }
        my $c = substr($src, $i, 1);
        if ($c eq "'") {                                       # char literal or lifetime
            if (substr($src, $i) =~ /\A('(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'\n])')/) {
                $out .= $1; $i += length $1; next;
            }
            $out .= $c; $i++; next;
        }
        if ($c eq '"') {                                       # normal / byte string
            my $j = $i + 1;
            while ($j < $n) {
                my $d = substr($src, $j, 1);
                if ($d eq '\\') { $j += 2; next }
                last if $d eq '"';
                $j++;
            }
            $out .= '"' . rebrand_text(substr($src, $i + 1, $j - $i - 1)) . '"';
            $i = $j + 1; next;
        }
        if ($c =~ /[A-Za-z0-9_]/) {                            # identifier/number: copy whole
            substr($src, $i) =~ /\A([A-Za-z0-9_]+)/;
            $out .= $1; $i += length $1; next;
        }
        $out .= $c; $i++;
    }
    return $out;
}

my ($files, $changed) = (0, 0);
find({ no_chdir => 1, wanted => sub {
    my $f = $File::Find::name;
    return unless -f $f;
    return unless $f =~ /\.rs$/ || $f =~ m{/i18n/[^/]+\.tsv$};
    return if $f =~ m{(^|/)(target|vendor)/};
    open my $fh, '<:raw', $f or die "$f: $!";
    local $/; my $src = <$fh>; close $fh;
    $files++;
    my $new = $f =~ /\.tsv$/ ? rebrand_text($src) : rebrand_rust($src);
    return if $new eq $src;
    open my $oh, '>:raw', $f or die "$f: $!"; print $oh $new; close $oh;
    $changed++;
}}, @ROOTS);
print "rebrand: $changed of $files files changed\n";
